use std::path::Path;

use anyhow::{anyhow, Context, Result};

/// Opens a file or folder with the OS default handler.
/// On Windows we use `explorer.exe` (folder browser) for directories and
/// the shell's "open" verb (via `cmd /c start ""`) for files.
pub fn open(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("path does not exist: {}", path.display()));
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        if path.is_dir() {
            Command::new("explorer.exe")
                .arg(path)
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .context("failed to launch explorer")?;
        } else {
            // `start` is a cmd builtin; the empty "" is a window title placeholder
            // so paths with spaces get parsed correctly.
            Command::new("cmd")
                .args(["/c", "start", ""])
                .arg(path)
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .context("failed to open file")?;
        }
    }

    #[cfg(not(windows))]
    {
        let _ = path; // silence unused warning on non-windows
        return Err(anyhow!("Windows only"));
    }

    Ok(())
}

/// Recursively removes everything inside `dir`, then recreates it empty.
/// `dir` itself is preserved so it can stay opened in Explorer.
pub fn purge_directory(dir: &Path) -> Result<()> {
    if !dir.exists() {
        std::fs::create_dir_all(dir).context("failed to create directory")?;
        return Ok(());
    }
    for entry in std::fs::read_dir(dir).context("failed to read directory")? {
        let entry = entry.context("failed to read directory entry")?;
        let p = entry.path();
        let r = if p.is_dir() {
            std::fs::remove_dir_all(&p)
        } else {
            std::fs::remove_file(&p)
        };
        r.with_context(|| format!("failed to delete: {}", p.display()))?;
    }
    Ok(())
}
