//! "Open AppContainer Loopback Utility" launcher.
//!
//! ATTRIBUTION & LICENSING
//! -----------------------
//! `EnableLoopback.exe` is a third-party utility distributed by
//! Telerik / Progress Software (Fiddler). Its license **forbids
//! redistribution**, so this project does **not** bundle the binary.
//! Instead we lazily download it directly from Telerik's CDN on first
//! use and hand off to it (or to the Telerik installer it bootstraps).
//!
//! WHY IT EXISTS HERE
//! ------------------
//! Windows 8+ AppContainer apps (UWP / Edge / Microsoft Store apps)
//! cannot connect to local loopback addresses by default; without an
//! exemption those apps silently fail to traffic through any local
//! proxy. The Loopback Utility is the standard one-click way to grant
//! per-app exemptions, which is essential when sing-box runs as an HTTP
//! proxy or mixed-inbound on 127.0.0.1.
//!
//! FLOW
//! ----
//! 1. If `C:\Program Files (x86)\EnableLoopback\EnableLoopback.exe`
//!    already exists (i.e. the user ran the Telerik installer in a
//!    previous session) → just shell-open it.
//! 2. Otherwise download the installer from
//!    <https://telerik-fiddler.s3.amazonaws.com/fiddler/addons/enableloopbackutility.exe>
//!    into `<install dir>/tools/` and shell-open it. The downloaded
//!    file carries Telerik's own UAC manifest, so a UAC prompt fires
//!    on its own.
//!
//! Network and disk I/O are blocking, so callers should invoke
//! [`open_or_download`] from the background worker thread, not the
//! UI thread.

#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::core::paths::Paths;
use crate::core::shell;

/// Display name shown in the Settings UI.
pub const DISPLAY_NAME: &str = "AppContainer loopback utility";

/// Hover-text attribution + behaviour summary for the settings row.
pub const HOVER_TEXT: &str = "Third-party utility from Telerik / Fiddler. \
                              Lets UWP / Edge / Store apps connect to local proxies. \
                              Downloaded from Telerik on first use; redistribution \
                              of the binary is forbidden by their license. UAC will prompt.";

/// Path the Telerik installer drops the actual utility at.
const INSTALLED_PATH: &str = r"C:\Program Files (x86)\EnableLoopback\EnableLoopback.exe";

/// Telerik's official download URL for the installer-style utility.
const DOWNLOAD_URL: &str =
    "https://telerik-fiddler.s3.amazonaws.com/fiddler/addons/enableloopbackutility.exe";

/// Filename used when caching the downloaded installer locally.
const INSTALLER_FILENAME: &str = "enableloopbackutility.exe";

/// Check the well-known install location and, if missing, download the
/// Telerik installer; then launch whatever is on disk via the shell so
/// UAC fires from the binary's own manifest.
pub fn open_or_download(paths: &Paths) -> std::io::Result<()> {
    let installed = Path::new(INSTALLED_PATH);
    if installed.exists() {
        return launch(installed);
    }

    let cached = ensure_downloaded(paths)?;
    launch(&cached)
}

fn launch(path: &Path) -> std::io::Result<()> {
    shell::open(path).map_err(|e| std::io::Error::other(e.to_string()))
}

fn ensure_downloaded(paths: &Paths) -> std::io::Result<PathBuf> {
    let tools_dir = paths.root.join("tools");
    std::fs::create_dir_all(&tools_dir)?;
    let dest = tools_dir.join(INSTALLER_FILENAME);

    // Reuse the cached installer only if it still looks like a Windows
    // executable. This avoids launching a cached HTML/error page or a
    // truncated prior download.
    if dest.exists() {
        if is_valid_pe_file(&dest)? {
            return Ok(dest);
        }
        std::fs::remove_file(&dest)?;
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("sing-box-for-windows")
        .build()
        .map_err(|e| std::io::Error::other(format!("HTTP client init failed: {e}")))?;

    let resp = client
        .get(DOWNLOAD_URL)
        .send()
        .map_err(|e| std::io::Error::other(format!("download request failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(std::io::Error::other(format!(
            "download failed: HTTP {status}"
        )));
    }

    let bytes = resp
        .bytes()
        .map_err(|e| std::io::Error::other(format!("download read failed: {e}")))?;

    validate_pe_bytes(&bytes)?;

    // Write atomically: tmp → rename. Avoids leaving a half-written
    // exe behind if the process is killed mid-write.
    let tmp = dest.with_extension("exe.partial");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &dest)?;
    Ok(dest)
}

fn is_valid_pe_file(path: &Path) -> std::io::Result<bool> {
    let bytes = std::fs::read(path)?;
    Ok(validate_pe_bytes(&bytes).is_ok())
}

fn validate_pe_bytes(bytes: &[u8]) -> std::io::Result<()> {
    if bytes.len() < 0x40 {
        return Err(std::io::Error::other("downloaded file is too small"));
    }
    if &bytes[..2] != b"MZ" {
        return Err(std::io::Error::other(
            "downloaded file is not a Windows executable",
        ));
    }

    let pe_offset = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
    let Some(pe_end) = pe_offset.checked_add(4) else {
        return Err(std::io::Error::other("invalid PE header offset"));
    };
    if pe_end > bytes.len() || &bytes[pe_offset..pe_end] != b"PE\0\0" {
        return Err(std::io::Error::other("invalid PE header signature"));
    }
    Ok(())
}
