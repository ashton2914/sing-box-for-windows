use std::path::Path;

use serde::{Deserialize, Serialize};

/// Global app settings persisted as `settings.json` next to the exe.
///
/// Per-config metadata (name, source URL/path, last_updated) lives inside
/// each config folder under `config/<slug>/metadata.json`, NOT here.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Folder slug (under `config/`) of the active config.
    pub selected_config: Option<String>,
    /// Filename inside `core/` that gets executed.
    pub selected_core: Option<String>,
    /// Auto launch sing-box when the launcher window opens.
    pub auto_start: bool,
    /// Periodically re-fetch the selected config from its source.
    pub auto_update: bool,
    pub update_interval_minutes: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            selected_config: None,
            selected_core: None,
            auto_start: false,
            auto_update: false,
            update_interval_minutes: 360,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string());
        std::fs::write(path, json)
    }
}
