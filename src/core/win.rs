//! Tiny shared Win32 helpers used by several modules.
//!
//! Anything that's not Win32-specific or that would pull a cross-platform
//! dependency belongs elsewhere — this file deliberately stays minimal so
//! it doesn't accumulate unrelated junk.

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

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

/// Re-enable Windows 11's rounded window corners on a frameless window.
///
/// When we create the viewport with `decorated: false` the OS removes
/// the entire non-client area, including the rounded-corner geometry
/// the DWM normally applies on Win11. The `DWMWA_WINDOW_CORNER_PREFERENCE`
/// attribute lets us opt back in. On Win10 the attribute is silently
/// ignored, so this is safe to call unconditionally on Windows.
pub fn enable_rounded_corners(hwnd: usize) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };

    let pref: i32 = DWMWCP_ROUND;
    unsafe {
        // HRESULT result is intentionally discarded — failure here is
        // purely cosmetic (square corners) and shouldn't break the app.
        let _ = DwmSetWindowAttribute(
            hwnd as HWND,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &pref as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );
    }
}
