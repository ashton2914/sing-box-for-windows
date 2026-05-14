//! Win32 system tray icon for the "Close to tray" feature.
//!
//! Spawns a dedicated worker thread that owns a hidden message-only
//! window plus a `Shell_NotifyIcon` entry. The worker directly owns the
//! tray popup behavior and directly shows the main Win32 window when the
//! user chooses Show.
//!
//! When [`TrayHandle`] is dropped (e.g. the user disables
//! "Close button hides to tray", or the app exits) the worker is
//! signalled, the tray icon is removed, and the worker thread joins.
//!
//! IMPORTANT: while the main window is hidden, eframe may stop calling
//! `App::update`, so the tray worker must not depend on the egui update
//! loop for Show/Exit. It receives the main window HWND from the UI
//! thread and calls Win32 APIs directly.

use std::cell::RefCell;
use std::mem::{size_of, zeroed};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;

use windows_sys::core::PCWSTR;
use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, EnumWindows, GetCursorPos, GetMessageW, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindow, LoadIconW, LoadImageW, PostMessageW, PostQuitMessage,
    RegisterClassExW, SetForegroundWindow, ShowWindow, TrackPopupMenu, TranslateMessage, HICON,
    HMENU, IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTCOLOR, LR_SHARED, MF_SEPARATOR, MF_STRING, MSG,
    SW_HIDE, SW_RESTORE, SW_SHOW, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_CLOSE,
    WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_USER, WNDCLASSEXW,
};

use crate::core::process::ProcessHandle;
use crate::core::win::wide;
use crate::log_bus::LogEvent;

/// Custom message id Shell_NotifyIcon will post back for icon mouse events.
const WM_TRAY_CALLBACK: u32 = WM_USER + 1;
/// Message used by the UI thread to hand the root eframe HWND to the tray worker.
const WM_SET_MAIN_HWND: u32 = WM_USER + 2;

/// Menu command ids. Kept small (<0x8000) per Win32 menu conventions.
const ID_MENU_SHOW: u32 = 0x4001;
const ID_MENU_EXIT: u32 = 0x4002;

/// Per-process unique id for the notification icon. We only ever own one.
const TRAY_ICON_UID: u32 = 1;

/// Per-thread state for the wnd_proc. Kept in a thread-local because the
/// `wnd_proc` callback is `extern "system"` and can't carry a closure
/// state pointer without manual `SetWindowLongPtrW` plumbing.
struct WorkerState {
    main_hwnd: Option<usize>,
    proc: Arc<ProcessHandle>,
    log_tx: std::sync::mpsc::Sender<LogEvent>,
}

thread_local! {
    static WORKER: RefCell<Option<WorkerState>> = const { RefCell::new(None) };
}

fn set_main_hwnd(hwnd: usize) {
    WORKER.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            state.main_hwnd = Some(hwnd);
        }
    });
}

fn show_main_window(tray_hwnd: HWND) {
    let hwnd = WORKER.with(|cell| {
        let mut worker = cell.borrow_mut();
        let state = worker.as_mut()?;
        if state.main_hwnd.is_none() {
            state.main_hwnd = find_main_window(tray_hwnd);
        }
        state.main_hwnd
    });

    if let Some(hwnd) = hwnd {
        unsafe {
            let hwnd = hwnd as HWND;
            if IsWindow(hwnd) != 0 {
                ShowWindow(hwnd, SW_SHOW);
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
        }
    }
}

fn exit_process(hwnd: HWND) -> ! {
    WORKER.with(|cell| {
        if let Some(state) = cell.borrow().as_ref() {
            let _ = state.proc.stop(state.log_tx.clone());
        }
    });
    unsafe {
        remove_tray_icon(hwnd);
    }
    std::process::exit(0);
}

/// Owned by the main app. Dropping it removes the tray icon and joins
/// the worker thread.
pub struct TrayHandle {
    /// HWND of the hidden tray window, stored as `usize` so the handle
    /// is `Send`. Guarded by [`IsWindow`] before posting to it.
    hwnd: usize,
    join: Option<thread::JoinHandle<()>>,
}

// HWND is `*mut c_void`, which is not `Send`. We only ever pass it back
// to Win32 APIs (PostMessageW / IsWindow), never dereference it on the
// Rust side, so it's safe to cross threads.
unsafe impl Send for TrayHandle {}

impl Drop for TrayHandle {
    fn drop(&mut self) {
        unsafe {
            if self.hwnd != 0 {
                let hwnd = self.hwnd as HWND;
                if IsWindow(hwnd) != 0 {
                    // wnd_proc handles WM_CLOSE → DestroyWindow → WM_DESTROY,
                    // which removes the tray icon and posts WM_QUIT.
                    PostMessageW(hwnd, WM_CLOSE, 0, 0);
                }
            }
        }
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl TrayHandle {
    /// Update the HWND of the real eframe root window. The tray worker
    /// uses this HWND directly for Show and Hide so it doesn't depend on
    /// the egui/winit update loop while hidden.
    pub fn set_main_hwnd(&self, hwnd: usize) {
        unsafe {
            let tray_hwnd = self.hwnd as HWND;
            if !tray_hwnd.is_null() && IsWindow(tray_hwnd) != 0 {
                PostMessageW(tray_hwnd, WM_SET_MAIN_HWND, hwnd, 0);
            }
        }
    }

    /// Hide the real eframe root window immediately via Win32.
    pub fn hide_main_window(&self, hwnd: usize) {
        unsafe {
            let hwnd = hwnd as HWND;
            if !hwnd.is_null() && IsWindow(hwnd) != 0 {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
}

/// Spawn the tray worker. Returns `None` if the hidden window or the
/// `Shell_NotifyIcon` registration fails (the caller should surface an
/// error to the user; the rest of the app keeps running).
pub fn spawn(
    proc: Arc<ProcessHandle>,
    log_tx: std::sync::mpsc::Sender<LogEvent>,
) -> Option<TrayHandle> {
    let (hwnd_tx, hwnd_rx) = channel::<usize>();

    let join = thread::Builder::new()
        .name("sing-box-tray".into())
        .spawn(move || unsafe {
            WORKER.with(|cell| {
                *cell.borrow_mut() = Some(WorkerState {
                    main_hwnd: None,
                    proc,
                    log_tx,
                });
            });

            let hwnd = match create_tray_window() {
                Some(h) => h,
                None => {
                    let _ = hwnd_tx.send(0);
                    return;
                }
            };

            if !add_tray_icon(hwnd) {
                DestroyWindow(hwnd);
                let _ = hwnd_tx.send(0);
                return;
            }

            let _ = hwnd_tx.send(hwnd as usize);

            // Standard Win32 message pump. Exits when wnd_proc posts
            // WM_QUIT (during WM_DESTROY).
            let mut msg: MSG = zeroed();
            loop {
                let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                if r <= 0 {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            WORKER.with(|cell| *cell.borrow_mut() = None);
        })
        .ok()?;

    let hwnd = hwnd_rx.recv().ok()?;
    if hwnd == 0 {
        let _ = join.join();
        return None;
    }

    Some(TrayHandle {
        hwnd,
        join: Some(join),
    })
}

unsafe fn create_tray_window() -> Option<HWND> {
    let hinstance = GetModuleHandleW(std::ptr::null());
    let class_name = wide("sing_box_for_windows_tray_class");

    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };

    // Re-registration after a previous spawn fails with
    // ERROR_CLASS_ALREADY_EXISTS, which is fine — the existing class
    // (same name + same hInstance + same wnd_proc) stays usable.
    let _ = RegisterClassExW(&wc);

    let title = wide("sing-box tray");
    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        0,
        0,
        0,
        0,
        0,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        hinstance,
        std::ptr::null(),
    );

    if hwnd.is_null() {
        None
    } else {
        Some(hwnd)
    }
}

unsafe fn add_tray_icon(hwnd: HWND) -> bool {
    let hinstance = GetModuleHandleW(std::ptr::null());

    // The build script (via winresource) embeds the multi-size .ico
    // under integer resource id 1. LoadImageW with `LR_SHARED` lets
    // Windows manage the lifetime so we don't need a matching
    // DestroyIcon.
    let mut hicon: HICON = LoadImageW(
        hinstance,
        1usize as PCWSTR,
        IMAGE_ICON,
        16,
        16,
        LR_DEFAULTCOLOR | LR_SHARED,
    ) as HICON;
    if hicon.is_null() {
        // Fallback so the tray entry still appears even if the embedded
        // resource is missing for some reason.
        hicon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
    }

    let mut nid: NOTIFYICONDATAW = zeroed();
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_UID;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY_CALLBACK;
    nid.hIcon = hicon;

    // Tooltip — szTip is a fixed [u16; 128] array, NUL-terminated.
    let tip = wide(crate::APP_TITLE);
    let n = tip.len().min(nid.szTip.len());
    nid.szTip[..n].copy_from_slice(&tip[..n]);

    Shell_NotifyIconW(NIM_ADD, &nid) != 0
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = zeroed();
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_UID;
    Shell_NotifyIconW(NIM_DELETE, &nid);
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY_CALLBACK => {
            // For Shell_NotifyIcon callbacks, the low word of lParam
            // carries the original mouse message.
            let mouse_msg = (lparam as u32) & 0xFFFF;
            match mouse_msg {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => show_main_window(hwnd),
                WM_RBUTTONUP => show_context_menu(hwnd),
                _ => {}
            }
            0
        }
        WM_SET_MAIN_HWND => {
            set_main_hwnd(wparam);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            remove_tray_icon(hwnd);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_context_menu(hwnd: HWND) {
    let menu: HMENU = CreatePopupMenu();
    if menu.is_null() {
        return;
    }

    let show_label = wide("Show");
    let exit_label = wide("Exit");
    AppendMenuW(menu, MF_STRING, ID_MENU_SHOW as usize, show_label.as_ptr());
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    AppendMenuW(menu, MF_STRING, ID_MENU_EXIT as usize, exit_label.as_ptr());

    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);

    // MSDN: must be the foreground window for the popup to dismiss
    // properly when the user clicks elsewhere.
    SetForegroundWindow(hwnd);
    // TPM_RETURNCMD makes TrackPopupMenu return the chosen command id
    // directly instead of posting a WM_COMMAND back to the owner.
    // Hidden tray windows do not reliably receive WM_COMMAND from a
    // popup menu (the message often gets dropped because the window
    // is not in the foreground input chain), so we read the result
    // synchronously here and dispatch ourselves.
    let cmd = TrackPopupMenu(
        menu,
        TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RETURNCMD,
        pt.x,
        pt.y,
        0,
        hwnd,
        std::ptr::null(),
    );
    // Required for hidden tray windows: TrackPopupMenu can fail to
    // dismiss its popup when the owning window is invisible. Posting a
    // benign message wakes the loop so the menu tears down cleanly.
    PostMessageW(hwnd, WM_NULL, 0, 0);

    DestroyMenu(menu);

    match cmd as u32 {
        ID_MENU_SHOW => show_main_window(hwnd),
        ID_MENU_EXIT => exit_process(hwnd),
        _ => {} // 0 — user dismissed the menu without choosing.
    }
}

struct MainWindowSearch {
    tray_hwnd: HWND,
    pid: u32,
    found: Option<usize>,
}

fn find_main_window(tray_hwnd: HWND) -> Option<usize> {
    let mut search = MainWindowSearch {
        tray_hwnd,
        pid: unsafe { GetCurrentProcessId() },
        found: None,
    };
    unsafe {
        EnumWindows(
            Some(enum_main_window_proc),
            &mut search as *mut MainWindowSearch as LPARAM,
        );
    }
    search.found
}

unsafe extern "system" fn enum_main_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let search = &mut *(lparam as *mut MainWindowSearch);
    if hwnd == search.tray_hwnd {
        return 1;
    }

    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid != search.pid {
        return 1;
    }

    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return 1;
    }
    let mut title = vec![0u16; len as usize + 1];
    let copied = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
    if copied <= 0 {
        return 1;
    }
    let title = String::from_utf16_lossy(&title[..copied as usize]);
    if title == crate::APP_TITLE {
        search.found = Some(hwnd as usize);
        return 0;
    }

    1
}
