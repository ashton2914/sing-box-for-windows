//! Per-config folder model.
//!
//! Layout:
//! ```text
//! config/
//!   <slug>/
//!     config.json        # the actual sing-box config passed to `-c`
//!     metadata.json      # name + source + last_updated
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::persist;

pub const CONFIG_FILENAME: &str = "config.json";
pub const METADATA_FILENAME: &str = "metadata.json";

/// Where a config originally came from. Used for the "Update" action.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Source {
    /// Imported from a file on disk. `path` is kept so re-import is possible.
    Local { path: PathBuf },
    /// Downloaded from a URL.
    Remote { url: String },
}

impl Source {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Source::Local { .. } => "Local",
            Source::Remote { .. } => "Remote",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigMetadata {
    pub name: String,
    pub source: Source,
    #[serde(default)]
    pub last_updated: Option<String>,
}

impl ConfigMetadata {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        serde_json::from_str(&raw)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        persist::save_json_pretty(path, self)
    }
}

/// A config folder loaded from disk.
#[derive(Clone, Debug)]
pub struct ConfigEntry {
    /// Folder name under `config/`. Used as the stable id.
    pub slug: String,
    pub metadata: ConfigMetadata,
    pub folder: PathBuf,
}

impl ConfigEntry {
    pub fn config_file(&self) -> PathBuf {
        self.folder.join(CONFIG_FILENAME)
    }

    pub fn metadata_file(&self) -> PathBuf {
        self.folder.join(METADATA_FILENAME)
    }
}

/// Generate a random 12-char lowercase hex folder slug.
///
/// Folder names are not user-visible (the display name lives in
/// `metadata.json`), so we just need something filesystem-safe and
/// unlikely to collide. 48 bits of entropy from a tiny xorshift PRNG
/// seeded by the system clock + a process-local counter is plenty.
pub fn random_slug() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut state = nanos ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    if state == 0 {
        state = 0xDEAD_BEEF_CAFE_BABE;
    }

    const HEX: &[u8; 16] = b"0123456789abcdef";
    const LEN: usize = 12;
    let mut out = String::with_capacity(LEN);
    for _ in 0..LEN {
        // xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push(HEX[(state as usize) & 0xF] as char);
    }
    out
}

/// Pick a random folder slug that doesn't collide with anything already
/// in `config_dir`.
pub fn unique_slug(config_dir: &Path) -> String {
    for _ in 0..1000 {
        let candidate = random_slug();
        if !config_dir.join(&candidate).exists() {
            return candidate;
        }
    }
    // 1000 collisions on a 48-bit space means something is very wrong;
    // fall back to whatever we get and let the caller surface the error.
    random_slug()
}
