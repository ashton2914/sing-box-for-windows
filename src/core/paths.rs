use std::path::{Path, PathBuf};

use crate::config::entry::{ConfigEntry, ConfigMetadata, METADATA_FILENAME};

/// All filesystem paths the launcher cares about, resolved relative to the
/// executable's directory (release) or the current working directory (debug).
#[derive(Clone, Debug)]
pub struct Paths {
    #[allow(dead_code)]
    pub root: PathBuf,
    pub core_dir: PathBuf,
    pub config_dir: PathBuf,
    pub working_dir: PathBuf,
    pub settings_file: PathBuf,
}

impl Paths {
    pub fn resolve() -> std::io::Result<Self> {
        let root = if cfg!(debug_assertions) {
            std::env::current_dir()?
        } else {
            let exe = std::env::current_exe()?;
            exe.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
        };

        let core_dir = root.join("core");
        let config_dir = root.join("config");
        let working_dir = root.join("sing-box");
        let settings_file = root.join("settings.json");

        std::fs::create_dir_all(&core_dir)?;
        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&working_dir)?;

        Ok(Self {
            root,
            core_dir,
            config_dir,
            working_dir,
            settings_file,
        })
    }

    pub fn config_folder(&self, slug: &str) -> PathBuf {
        self.config_dir.join(slug)
    }

    /// Load a single `ConfigEntry` by slug. Cheaper than `list_configs`
    /// for the common "operate on one entry" path (update / edit /
    /// delete) — reads exactly one `metadata.json` instead of scanning
    /// every sub-folder in `config/`.
    pub fn load_entry(&self, slug: &str) -> Option<ConfigEntry> {
        let folder = self.config_folder(slug);
        if !folder.is_dir() {
            return None;
        }
        let metadata = ConfigMetadata::load(&folder.join(METADATA_FILENAME)).ok()?;
        Some(ConfigEntry {
            slug: slug.to_string(),
            metadata,
            folder,
        })
    }

    pub fn core_path(&self, name: &str) -> PathBuf {
        self.core_dir.join(name)
    }

    /// All sub-folders of `config/` that contain a `metadata.json`.
    /// Returned sorted by display name.
    pub fn list_configs(&self) -> Vec<ConfigEntry> {
        let mut out: Vec<ConfigEntry> = std::fs::read_dir(&self.config_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|e| {
                let folder = e.path();
                let slug = folder.file_name()?.to_str()?.to_string();
                let meta_path = folder.join(METADATA_FILENAME);
                let metadata = ConfigMetadata::load(&meta_path).ok()?;
                Some(ConfigEntry {
                    slug,
                    metadata,
                    folder,
                })
            })
            .collect();
        out.sort_by(|a, b| {
            a.metadata
                .name
                .to_lowercase()
                .cmp(&b.metadata.name.to_lowercase())
        });
        out
    }

    /// All `*.exe` files directly inside `core/`.
    pub fn list_cores(&self) -> Vec<String> {
        list_files_with_ext(&self.core_dir, "exe")
    }
}

fn list_files_with_ext(dir: &Path, ext: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let p = e.path();
            let matches_ext = p
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case(ext))
                .unwrap_or(false);
            if !matches_ext {
                return None;
            }
            p.file_name().and_then(|n| n.to_str()).map(|s| s.to_owned())
        })
        .collect();
    out.sort_unstable();
    out
}
