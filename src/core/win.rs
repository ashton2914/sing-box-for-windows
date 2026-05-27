//! Tiny shared Win32 helpers used by several modules.
//!
//! Anything that's not Win32-specific or that would pull a cross-platform
//! dependency belongs elsewhere — this file deliberately stays minimal so
//! it doesn't accumulate unrelated junk.

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, SetWindowLongPtrW, ShowWindow, GWLP_WNDPROC, SW_HIDE,
    WM_CLOSE, WNDPROC,
};

/// `CREATE_NO_WINDOW` process creation flag — prevents a black conhost
/// window from flashing when the launcher spawns child processes
/// (`reg.exe`, `schtasks.exe`, `cmd /c start`, the sing-box core itself).
///
/// Defined here once so every spawn site agrees on the value; previous
/// per-module copies drifted between `0x0800_0000` and `0x08000000`
/// (same number, different formatting — a recipe for a future typo).
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Encode an `OsStr` to a NUL-terminated UTF-16 buffer suitable for the
/// `*W` Win32 APIs. Used by tray (`CreateWindowExW`, `Shell_NotifyIconW`,
/// menu strings) and elevation (`ShellExecuteExW`, `CreateProcessW`).
pub fn to_wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// `&str` convenience wrapper over [`to_wide`] for the common case of a
/// hard-coded UTF-8 literal.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---------------------------------------------------------------------------
// Main-window WM_CLOSE subclass
// ---------------------------------------------------------------------------
//
// Problem this solves: Windows suppresses `WM_PAINT` / `RedrawRequested`
// for **iconic** (minimized) windows. winit honors that suppression, so
// `App::update` does not run while the window is minimized. Right-clicking
// the taskbar entry of a minimized window and choosing Close therefore
// queues a `WM_CLOSE` that winit never gets to convert into a
// `CloseRequested` event the Rust side can act on — the click feels dead.
//
// Fix: install a wndproc subclass on the main eframe window. The subclass
// runs *before* winit's wndproc for every message. For `WM_CLOSE`, if
// "close button hides to tray" is on, it calls `ShowWindow(SW_HIDE)`
// directly and returns 0 (consumed) — no Rust-side event loop needed.
// For every other message (and for `WM_CLOSE` when the setting is off,
// i.e. the user wants a real exit) it chains to the original wndproc via
// `CallWindowProcW`, so winit sees the world unchanged.
//
// The subclass also reads from an `Arc<AtomicBool>` shared with the UI
// thread, which keeps the live value of the setting in sync without
// re-installing anything when the user toggles it.

static CLOSE_TO_TRAY: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// Original wndproc returned by `SetWindowLongPtrW`, stored as `isize`
/// (the raw API return type) so it lives in an atomic. Zero means
/// "subclass not installed yet"; once non-zero it never changes.
static ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);

/// Install the close-to-tray subclass on the given HWND. Safe to call
/// multiple times — only the first call replaces the wndproc; subsequent
/// calls are no-ops. The `close_to_tray` flag is consulted on every
/// `WM_CLOSE` and reflects the live setting.
pub fn install_close_to_tray_subclass(hwnd: usize, close_to_tray: Arc<AtomicBool>) {
    if hwnd == 0 {
        return;
    }
    // First-call wins for the shared flag — subsequent calls keep
    // pointing at the same `Arc<AtomicBool>` so mutations from the UI
    // thread are visible to the subclass.
    let _ = CLOSE_TO_TRAY.set(close_to_tray);

    if ORIGINAL_WNDPROC.load(Ordering::Acquire) != 0 {
        return; // Already installed.
    }

    // SAFETY: `SetWindowLongPtrW(GWLP_WNDPROC, ...)` replaces the
    // window's wndproc and returns the previous one. We store it so
    // the subclass can chain via `CallWindowProcW`. winit's own per-
    // window state lives in `GWLP_USERDATA` which we do not touch,
    // so winit keeps working exactly as before for every message we
    // forward.
    unsafe {
        let prev =
            SetWindowLongPtrW(hwnd as HWND, GWLP_WNDPROC, subclass_wndproc as *const () as isize);
        ORIGINAL_WNDPROC.store(prev, Ordering::Release);
    }
}

unsafe extern "system" fn subclass_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CLOSE {
        if let Some(flag) = CLOSE_TO_TRAY.get() {
            if flag.load(Ordering::Relaxed) {
                // Consume the close: hide the window. The tray worker
                // already knows how to re-show it via Show / double-
                // click. We deliberately do NOT chain to the original
                // wndproc here — that would let winit see the close
                // and either start tearing down or post duplicate
                // events.
                ShowWindow(hwnd, SW_HIDE);
                return 0;
            }
        }
    }

    let original = ORIGINAL_WNDPROC.load(Ordering::Acquire);
    if original != 0 {
        // SAFETY: `original` came from `SetWindowLongPtrW(GWLP_WNDPROC,
        // ...)` so it is a valid `WNDPROC` for this HWND.
        let original_fn: WNDPROC = std::mem::transmute(original);
        CallWindowProcW(original_fn, hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
