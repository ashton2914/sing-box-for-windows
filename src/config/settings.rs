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
    /// Launch this app automatically when Windows starts.
    pub auto_start: bool,
    /// Auto launch the selected sing-box core when this app starts.
    pub auto_start_sing_box: bool,
    /// Persistently relaunch as Administrator on every start (silent,
    /// no UAC prompt) by chaining through a registered Task Scheduler
    /// task. The task itself is created/removed by
    /// `core::elevation::ensure_admin_task` / `delete_admin_task`.
    pub always_admin: bool,
    /// Periodically re-fetch the selected config from its source.
    pub auto_update: bool,
    pub update_interval_hours: u64,
    /// When `true`, clicking the window's close (X) button hides the
    /// launcher to the system tray instead of exiting. The user must
    /// right-click the tray icon and pick "Exit" to actually quit.
    pub close_to_tray: bool,
    /// When `true`, the app starts hidden and is available from the
    /// system tray immediately.
    pub silent_start: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            selected_config: None,
            selected_core: None,
            auto_start: false,
            auto_start_sing_box: false,
            always_admin: false,
            auto_update: false,
            update_interval_hours: 24,
            close_to_tray: false,
            silent_start: false,
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
