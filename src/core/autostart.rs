//! Windows "launch on user login" toggle.
//!
//! Implemented by writing the launcher's full path to:
//!     HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run\sing-box-for-windows
//!
//! Windows reads that key for the *current* user (no admin needed) and
//! launches every listed value when the user signs in. We shell out to
//! the built-in `reg.exe` so we don't take a winreg dependency.
//!
//! Operations are best-effort: failures bubble up as `io::Result` so the
//! UI can surface them, but a missing key on `is_enabled()` is treated as
//! "disabled", not an error.

use std::io;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::core::win::CREATE_NO_WINDOW;

/// Name we register under HKCU\…\Run. Stable across upgrades.
const REG_VALUE: &str = "sing-box-for-windows";
/// Legacy registry value name used by the launcher's first iteration.
/// Kept here only so [`migrate_legacy`] can clean it up.
const LEGACY_REG_VALUES: &[&str] = &["sing-box-launcher"];
const REG_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

fn reg_command() -> Command {
    let mut c = Command::new("reg.exe");
    c.creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    c
}

/// Path to the running launcher executable, used as the registry value.
fn launcher_path() -> io::Result<PathBuf> {
    std::env::current_exe()
}

/// True if the autostart entry currently points at *this* exe.
#[allow(dead_code)] // Reserved for future "detect external removal" UI.
pub fn is_enabled() -> bool {
    let Ok(want) = launcher_path() else {
        return false;
    };
    let want_norm = want.to_string_lossy().to_lowercase();

    let output = reg_command()
        .args(["query", REG_KEY, "/v", REG_VALUE])
        .stdout(Stdio::piped())
        .output();
    let Ok(out) = output else { return false };
    if !out.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&out.stdout).to_lowercase();
    // The query output contains the value name, type, and data on one
    // line. Just check that our exe path appears anywhere in it.
    stdout.contains(&want_norm) || stdout.contains(&want_norm.replace('/', "\\"))
}

/// Mirror `enabled` to the registry. Idempotent — safe to call from
/// `persist_settings` on every save.
pub fn set_enabled(enabled: bool) -> io::Result<()> {
    if enabled {
        let path = launcher_path()?;
        let path_str = path.to_string_lossy().into_owned();
        let status = reg_command()
            .args([
                "add", REG_KEY, "/v", REG_VALUE, "/t", "REG_SZ", "/d", &path_str, "/f",
            ])
            .status()?;
        if !status.success() {
            return Err(io::Error::new(io::ErrorKind::Other, "reg add failed"));
        }
    } else {
        // Delete the value if present; treat "not found" as success.
        let output = reg_command()
            .args(["delete", REG_KEY, "/v", REG_VALUE, "/f"])
            .stderr(Stdio::piped())
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("unable to find") && !stderr.contains("cannot find") {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("reg delete failed: {}", stderr.trim()),
                ));
            }
        }
    }
    Ok(())
}

/// Best-effort: remove any HKCU\\\u2026\\Run entries left over from
/// previously-deprecated launcher names so the user doesn't end up with
/// stale autostart pointing at an old exe path. Failures are silently
/// ignored \u2014 a missing legacy value is the common case.
pub fn migrate_legacy() {
    for legacy in LEGACY_REG_VALUES {
        let _ = reg_command()
            .args(["delete", REG_KEY, "/v", legacy, "/f"])
            .stderr(Stdio::null())
            .status();
    }
}
