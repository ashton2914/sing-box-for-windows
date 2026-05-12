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

    #[allow(dead_code)]
    pub fn detail(&self) -> String {
        match self {
            Source::Local { path } => path.display().to_string(),
            Source::Remote { url } => url.clone(),
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
        let json = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string());
        std::fs::write(path, json)
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

/// Generate a filesystem-safe folder slug from a user-given name.
/// Strips reserved characters and collapses whitespace into dashes.
pub fn slug_from_name(name: &str) -> String {
    let mut buf = String::with_capacity(name.len());
    let mut prev_dash = false;
    for c in name.chars() {
        let ch = if c.is_whitespace() {
            '-'
        } else if matches!(
            c,
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '.' | ','
        ) || c.is_control()
        {
            '-'
        } else {
            c
        };
        if ch == '-' {
            if !prev_dash {
                buf.push('-');
            }
            prev_dash = true;
        } else {
            buf.push(ch.to_ascii_lowercase());
            prev_dash = false;
        }
    }
    let trimmed = buf.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "config".to_string()
    } else {
        trimmed
    }
}

/// Pick a slug that doesn't collide with any folder already in `config_dir`.
/// Appends `-2`, `-3`, … as needed.
pub fn unique_slug(config_dir: &Path, name: &str) -> String {
    let base = slug_from_name(name);
    if !config_dir.join(&base).exists() {
        return base;
    }
    for n in 2..=u32::MAX {
        let candidate = format!("{base}-{n}");
        if !config_dir.join(&candidate).exists() {
            return candidate;
        }
    }
    base
}
