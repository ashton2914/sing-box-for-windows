//! UAC elevation helpers.
//!
//! `is_elevated()` queries the current process token to check whether
//! we already hold an elevated (administrator) integrity level — TUN
//! mode in sing-box requires this.
//!
//! `restart_as_admin()` spawns a fresh copy of the launcher via the
//! Windows shell with the `runas` verb. That triggers a UAC prompt; on
//! consent a new elevated process starts and the caller should then
//! shut down its own sing-box child and `process::exit(0)`. On UAC
//! cancel an `Err(PermissionDenied)` is returned and we stay running.
//!
//! We use `ShellExecuteExW` directly (via `windows-sys`) instead of
//! shelling out to PowerShell so we can detect cancel vs. real failures
//! and avoid a console flash.

use std::io;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::core::win::{to_wide, CREATE_NO_WINDOW};

/// `ERROR_CANCELLED` — UAC dialog dismissed by the user.
const ERROR_CANCELLED: u32 = 1223;

/// Per-user scheduled-task name used for the silent admin promotion.
/// Stable across upgrades. Stored in the user's task folder so it can be
/// created and deleted without admin (only the *registration* of an
/// elevated task itself requires admin).
pub const ADMIN_TASK_NAME: &str = "sing-box-for-windows-elevated";
/// Legacy scheduled task name used by the launcher's first iteration.
/// Kept only so [`migrate_legacy_admin_task`] can clean it up.
const LEGACY_ADMIN_TASK_NAMES: &[&str] = &["sing-box-launcher-elevated"];

/// True when the current process token has an elevated integrity level
/// (i.e. is running as Administrator).
///
/// Cached after the first call: a process's integrity level is fixed at
/// `CreateProcess` time and Windows offers no API to change it in
/// place, so the answer is invariant for the lifetime of the process.
/// The previous per-frame call hit `OpenProcessToken` +
/// `GetTokenInformation` on every UI repaint.
pub fn is_elevated() -> bool {
    use std::sync::OnceLock;
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(query_elevation)
}

fn query_elevation() -> bool {
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
        let mut ret_len: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

/// Spawn a fresh elevated copy of the launcher. Triggers a UAC prompt.
///
/// `working_dir` is passed as the new process's initial current
/// directory. Pass the same root your `Paths` resolver uses so
/// `settings.json` and `config/` are found by the elevated copy too —
/// otherwise UAC will hand the elevated process a `system32` cwd and
/// it'll silently fall back to default settings.
///
/// Returns `Ok(())` once the elevated process has been launched (caller
/// is responsible for cleanly shutting itself down). Returns
/// `Err(PermissionDenied)` if the user cancels UAC, or another `io::Error`
/// for unexpected shell failures.
pub fn restart_as_admin(working_dir: &Path) -> io::Result<()> {
    let exe = std::env::current_exe()?;

    let exe_w = to_wide(exe.as_os_str());
    let dir_w = to_wide(working_dir.as_os_str());
    let verb_w = to_wide(std::ffi::OsStr::new("runas"));

    let mut sei: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    sei.fMask = SEE_MASK_NOCLOSEPROCESS;
    sei.lpVerb = verb_w.as_ptr();
    sei.lpFile = exe_w.as_ptr();
    sei.lpDirectory = dir_w.as_ptr();
    sei.nShow = SW_SHOWNORMAL;

    let ok = unsafe { ShellExecuteExW(&mut sei) };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        let kind = if err == ERROR_CANCELLED {
            io::ErrorKind::PermissionDenied
        } else {
            io::ErrorKind::Other
        };
        return Err(io::Error::new(
            kind,
            format!("ShellExecuteExW failed (error {err})"),
        ));
    }
    if !sei.hProcess.is_null() && sei.hProcess != INVALID_HANDLE_VALUE {
        unsafe {
            CloseHandle(sei.hProcess);
        }
    }
    Ok(())
}

/// Relaunch the launcher as a *standard user*, dropping the current
/// process's elevated token. Windows doesn't permit a process to lower
/// its own integrity level, so the only supported trick is to spawn a
/// new process whose *parent* is a medium-integrity process — the
/// child inherits the parent's integrity level. The user's own
/// `explorer.exe` is always available during an interactive session
/// and runs at medium integrity, so it makes the perfect parent.
///
/// This is the same technique PowerToys uses in `RunNonElevated`. It
/// requires no token duplication and no special privileges beyond
/// `PROCESS_CREATE_PROCESS` access on Explorer (which an admin token
/// always has against a same-user process).
///
/// `working_dir` is passed as the new process's initial cwd, mirroring
/// `restart_as_admin` so the standard-user copy finds `settings.json`.
///
/// Note on the previous attempts:
///   * `ShellExecuteExW(explorer.exe, "<our.exe>")` — Windows silently
///     interprets this as "open a folder window", the exe is never
///     launched.
///   * `CreateProcessWithTokenW` with Explorer's duplicated token —
///     returns `ERROR_ACCESS_DENIED` (5) when `SeImpersonatePrivilege`
///     is filtered out of the admin token.
///   * `CreateProcessAsUserW` — needs `SeAssignPrimaryTokenPrivilege`,
///     which is *not* in admin tokens at all (only SYSTEM holds it).
pub fn restart_as_standard_user(working_dir: &Path) -> io::Result<()> {
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        OpenProcess, UpdateProcThreadAttribute, CREATE_UNICODE_ENVIRONMENT,
        EXTENDED_STARTUPINFO_PRESENT, PROCESS_CREATE_PROCESS, PROCESS_INFORMATION, STARTUPINFOEXW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetShellWindow, GetWindowThreadProcessId};

    /// `PROC_THREAD_ATTRIBUTE_PARENT_PROCESS` — not exported by name in
    /// windows-sys 0.59. Stable Win32 constant: composed from
    /// `ProcThreadAttributeValue(ProcThreadAttributeParentProcess=0,
    ///  Thread=FALSE, Input=TRUE, Additive=FALSE)` = `0x00020000`.
    const PROC_THREAD_ATTRIBUTE_PARENT_PROCESS: usize = 0x00020000;

    let exe = std::env::current_exe()?;

    unsafe {
        // 1. Locate explorer.exe via the current desktop's shell window.
        let hwnd = GetShellWindow();
        if hwnd.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "GetShellWindow returned NULL — no Explorer shell available \
                 (running under a non-interactive session?)",
            ));
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Could not determine Explorer PID from shell window",
            ));
        }

        // 2. Open Explorer with PROCESS_CREATE_PROCESS so we can name
        //    it as the parent in the proc-thread attribute list.
        let h_explorer = OpenProcess(PROCESS_CREATE_PROCESS, 0, pid);
        if h_explorer.is_null() {
            let err = GetLastError();
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "OpenProcess(explorer.exe pid={pid}, PROCESS_CREATE_PROCESS) \
                     failed (error {err})"
                ),
            ));
        }

        // 3. Allocate the proc-thread attribute list. The first call
        //    intentionally fails with ERROR_INSUFFICIENT_BUFFER and
        //    writes the required size into `size`.
        let mut size: usize = 0;
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
        if size == 0 {
            CloseHandle(h_explorer);
            return Err(io::Error::other(
                "InitializeProcThreadAttributeList failed to report size",
            ));
        }
        let mut attr_buf: Vec<u8> = vec![0u8; size];
        let attr_list = attr_buf.as_mut_ptr() as *mut core::ffi::c_void;

        if InitializeProcThreadAttributeList(attr_list, 1, 0, &mut size) == 0 {
            let err = GetLastError();
            CloseHandle(h_explorer);
            return Err(io::Error::other(format!(
                "InitializeProcThreadAttributeList(2) failed (error {err})"
            )));
        }

        // 4. Add the PROC_THREAD_ATTRIBUTE_PARENT_PROCESS entry. The
        //    value pointer must point at storage that lives until
        //    CreateProcessW returns — we keep `h_explorer` on the stack.
        let h_explorer_ptr: *const HANDLE = &h_explorer;
        let ok = UpdateProcThreadAttribute(
            attr_list,
            0,
            PROC_THREAD_ATTRIBUTE_PARENT_PROCESS,
            h_explorer_ptr as *const core::ffi::c_void,
            std::mem::size_of::<HANDLE>(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        if ok == 0 {
            let err = GetLastError();
            DeleteProcThreadAttributeList(attr_list);
            CloseHandle(h_explorer);
            return Err(io::Error::other(format!(
                "UpdateProcThreadAttribute(PARENT_PROCESS) failed (error {err})"
            )));
        }

        // 5. Spawn our exe with Explorer as its parent. The child
        //    inherits Explorer's medium-integrity token automatically;
        //    no UAC, no token-duplication, no special privileges.
        let exe_w = to_wide(exe.as_os_str());
        let dir_w = to_wide(working_dir.as_os_str());
        // CreateProcessW writes through lpCommandLine, so the buffer
        // must be a writable Vec<u16>.
        let cmdline = format!("\"{}\"", exe.display());
        let mut cmdline_w: Vec<u16> = to_wide(std::ffi::OsStr::new(&cmdline));

        let mut six: STARTUPINFOEXW = std::mem::zeroed();
        six.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        six.lpAttributeList = attr_list;
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();

        let ok = CreateProcessW(
            exe_w.as_ptr(),
            cmdline_w.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0, // bInheritHandles = FALSE
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            std::ptr::null(),
            dir_w.as_ptr(),
            &six as *const _ as *const _,
            &mut pi,
        );
        let err = GetLastError();

        DeleteProcThreadAttributeList(attr_list);
        CloseHandle(h_explorer);

        if ok == 0 {
            return Err(io::Error::other(format!(
                "CreateProcessW(parent=explorer) failed (error {err})"
            )));
        }
        if !pi.hThread.is_null() {
            CloseHandle(pi.hThread);
        }
        if !pi.hProcess.is_null() {
            CloseHandle(pi.hProcess);
        }
    }
    Ok(())
}

// -------------------------------------------------------------------------
// Persistent admin promotion via Task Scheduler.
//
// Windows refuses to launch HKCU\...\Run entries with elevated tokens —
// that mechanism is wired through the standard interactive-logon flow
// which always strips admin rights from the resulting process. The only
// supported way to get a silent, no-UAC admin auto-launch is to register
// a Task Scheduler task with RunLevel=HighestAvailable. PowerToys uses
// the same approach for its "Run as administrator" persistence.
//
// We register an *on-demand* task (no triggers) so that:
//   * it never auto-fires by itself (the HKCU\...\Run entry remains the
//     visible/disable-able startup item in Task Manager), and
//   * a standard-user instance of the launcher can call `schtasks /run`
//     on it to silently spawn an elevated copy of itself, then exit.
//
// Creating the task requires admin (because of the HIGHEST run level);
// running and deleting it do not, since the current user owns it.
// -------------------------------------------------------------------------

/// Build a `Command` for `schtasks.exe` with all stdio swallowed and the
/// console window suppressed. We always go through `schtasks` rather
/// than the native COM API to avoid pulling in extra dependencies.
fn schtasks_command() -> Command {
    let mut c = Command::new("schtasks.exe");
    c.creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    c
}

/// True when the persistent admin promotion task is registered.
pub fn admin_task_exists() -> bool {
    let out = schtasks_command()
        .args(["/query", "/tn", ADMIN_TASK_NAME])
        .stdout(Stdio::null())
        .output();
    matches!(out, Ok(o) if o.status.success())
}

/// Create or refresh the persistent admin task pointing at the current
/// exe. Requires the calling process to already be elevated, since
/// `RunLevel=HighestAvailable` task registration is admin-only.
///
/// `working_dir` is baked into the task's `<WorkingDirectory>` so the
/// elevated copy starts in the same directory as the parent (matters
/// in dev where `settings.json` lives at the source-tree root, not
/// next to the exe).
///
/// `with_logon_trigger` controls whether the task fires automatically
/// at user logon. Pass `true` when the user wants "launch on Windows
/// startup" *and* persistent admin — the task then replaces the
/// `HKCU\...\Run` entry, avoiding a dual launch.
pub fn ensure_admin_task(working_dir: &Path, with_logon_trigger: bool) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let xml = build_task_xml(&exe, working_dir, with_logon_trigger);

    // Write the XML to a temp file and pass it via /xml — `schtasks`
    // can't take XML on stdin and our path may contain quotes.
    // The file MUST be UTF-16 LE with a BOM, matching the
    // `<?xml encoding="UTF-16"?>` declaration; schtasks rejects UTF-8.
    let tmp = std::env::temp_dir().join("sing-box-for-windows-task.xml");
    let mut bytes: Vec<u8> = vec![0xFF, 0xFE]; // UTF-16 LE BOM
    for unit in xml.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    std::fs::write(&tmp, &bytes)?;

    let out = schtasks_command()
        .args([
            "/create",
            "/tn",
            ADMIN_TASK_NAME,
            "/xml",
            tmp.to_string_lossy().as_ref(),
            "/f",
        ])
        .stderr(Stdio::piped())
        .output()?;
    let _ = std::fs::remove_file(&tmp);

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(io::Error::other(format!(
            "schtasks /create failed: {}",
            stderr.trim()
        )));
    }
    Ok(())
}

/// Trigger the persistent admin task to spawn an elevated copy of the
/// launcher. Does not require admin in the calling process. Returns
/// `Err(NotFound)` if the task has not been registered yet.
pub fn run_admin_task() -> io::Result<()> {
    if !admin_task_exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Scheduled task '{}' is not registered", ADMIN_TASK_NAME),
        ));
    }
    let out = schtasks_command()
        .args(["/run", "/tn", ADMIN_TASK_NAME])
        .stderr(Stdio::piped())
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(io::Error::other(format!(
            "schtasks /run failed: {}",
            stderr.trim()
        )));
    }
    Ok(())
}

/// Delete the persistent admin task. Treats "task not found" as success
/// so it is safe to call when toggling the feature off from any state.
pub fn delete_admin_task() -> io::Result<()> {
    let out = schtasks_command()
        .args(["/delete", "/tn", ADMIN_TASK_NAME, "/f"])
        .stderr(Stdio::piped())
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
        if !stderr.contains("does not exist")
            && !stderr.contains("cannot find")
            && !stderr.contains("unable to find")
        {
            return Err(io::Error::other(format!(
                "schtasks /delete failed: {}",
                stderr.trim()
            )));
        }
    }
    Ok(())
}

/// Best-effort cleanup of scheduled tasks left over from previously
/// deprecated launcher names. A missing legacy task is the common case;
/// failures are silently ignored.
pub fn migrate_legacy_admin_task() {
    for legacy in LEGACY_ADMIN_TASK_NAMES {
        let _ = schtasks_command()
            .args(["/delete", "/tn", legacy, "/f"])
            .stderr(Stdio::null())
            .status();
    }
}

/// Generate the Task Scheduler XML used by `ensure_admin_task`.
///
/// The schema below describes a task that:
///   * runs as the current interactive user with `HighestAvailable`
///     elevation (silently consumes the user's admin token without UAC),
///   * is single-instance (`IgnoreNew`) so a `schtasks /run` while a
///     previous launch is still alive won't double up,
///   * launches the supplied exe with no arguments,
///   * if `with_logon_trigger` is true, also fires automatically at
///     user logon — effectively replacing the role of an `HKCU\...\Run`
///     entry but with elevation. This matches the PowerToys autostart
///     pattern visible in Task Scheduler → PowerToys → "Autorun for ...".
fn build_task_xml(
    exe: &std::path::Path,
    working_dir: &std::path::Path,
    with_logon_trigger: bool,
) -> String {
    let user_id = std::env::var("USERDOMAIN")
        .ok()
        .and_then(|d| std::env::var("USERNAME").ok().map(|u| format!("{d}\\{u}")))
        .or_else(|| std::env::var("USERNAME").ok())
        .unwrap_or_else(|| "S-1-5-18".to_string());

    let exe_xml = xml_escape(&exe.display().to_string());
    let dir_xml = xml_escape(&working_dir.display().to_string());
    let user_xml = xml_escape(&user_id);

    let triggers_xml = if with_logon_trigger {
        format!(
            "  <Triggers>\n    <LogonTrigger>\n      <Enabled>true</Enabled>\n      <UserId>{user_xml}</UserId>\n    </LogonTrigger>\n  </Triggers>\n"
        )
    } else {
        String::new()
    };

    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.3" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Silent admin promotion for sing-box-for-windows.</Description>
  </RegistrationInfo>
{triggers_xml}  <Principals>
    <Principal id="Author">
      <UserId>{user_xml}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <DisallowStartOnRemoteAppSession>false</DisallowStartOnRemoteAppSession>
    <UseUnifiedSchedulingEngine>true</UseUnifiedSchedulingEngine>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe_xml}</Command>
      <WorkingDirectory>{dir_xml}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
