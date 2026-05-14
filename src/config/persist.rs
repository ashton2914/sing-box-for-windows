use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub fn save_json_pretty<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to serialize JSON for {}: {e}", path.display()),
        )
    })?;
    write_atomic_with_backup(path, &bytes)
}

pub fn write_atomic_with_backup(dest: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "failed to create destination directory {}: {e}",
                    parent.display()
                ),
            )
        })?;
    }

    let tmp = with_suffix(dest, ".tmp");
    let bak = with_suffix(dest, ".bak");

    std::fs::write(&tmp, bytes).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("failed to write temp file {}: {e}", tmp.display()),
        )
    })?;

    if bak.exists() {
        if let Err(e) = std::fs::remove_file(&bak) {
            let _ = std::fs::remove_file(&tmp);
            return Err(io::Error::new(
                e.kind(),
                format!("failed to remove backup file {}: {e}", bak.display()),
            ));
        }
    }

    let had_dest = dest.exists();
    if had_dest {
        if let Err(e) = std::fs::rename(dest, &bak) {
            let _ = std::fs::remove_file(&tmp);
            return Err(io::Error::new(
                e.kind(),
                format!(
                    "failed to move existing file {} to backup {}: {e}",
                    dest.display(),
                    bak.display()
                ),
            ));
        }
    }

    if let Err(e) = std::fs::rename(&tmp, dest) {
        let _ = std::fs::remove_file(&tmp);
        if had_dest && !dest.exists() && bak.exists() {
            let _ = std::fs::rename(&bak, dest);
        }
        return Err(io::Error::new(
            e.kind(),
            format!(
                "failed to replace {} with temp file {}: {e}",
                dest.display(),
                tmp.display()
            ),
        ));
    }

    Ok(())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}
