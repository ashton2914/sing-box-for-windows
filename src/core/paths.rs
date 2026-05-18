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

    pub fn runtime_config_file(&self) -> PathBuf {
        self.working_dir.join("config.json")
    }

    /// All sub-folders of `config/` that contain a `metadata.json`, returned
    /// sorted by display name, plus any metadata/load errors that would
    /// otherwise make config folders disappear silently from the UI.
    pub fn list_configs_with_errors(&self) -> (Vec<ConfigEntry>, Vec<String>) {
        let mut out = Vec::new();
        let mut errors = Vec::new();

        let entries = match std::fs::read_dir(&self.config_dir) {
            Ok(entries) => entries,
            Err(e) => {
                errors.push(format!(
                    "Failed to read config directory {}: {e}",
                    self.config_dir.display()
                ));
                return (out, errors);
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    errors.push(format!("Failed to read config directory entry: {e}"));
                    continue;
                }
            };

            match entry.file_type() {
                Ok(t) if t.is_dir() => {}
                Ok(_) => continue,
                Err(e) => {
                    errors.push(format!(
                        "Failed to inspect config path {}: {e}",
                        entry.path().display()
                    ));
                    continue;
                }
            }

            let folder = entry.path();
            let Some(slug) = folder
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_owned)
            else {
                errors.push(format!(
                    "Skipped config folder with non-UTF-8 name: {}",
                    folder.display()
                ));
                continue;
            };
            let meta_path = folder.join(METADATA_FILENAME);
            match ConfigMetadata::load(&meta_path) {
                Ok(metadata) => out.push(ConfigEntry {
                    slug,
                    metadata,
                    folder,
                }),
                Err(e) => errors.push(format!(
                    "Failed to load config metadata {}: {e}",
                    meta_path.display()
                )),
            }
        }

        out.sort_by(|a, b| {
            a.metadata
                .name
                .to_lowercase()
                .cmp(&b.metadata.name.to_lowercase())
        });
        (out, errors)
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
