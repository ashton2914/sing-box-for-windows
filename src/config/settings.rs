use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::persist;

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
    /// User preference for the visual theme. Defaults to following the
    /// host OS (`ThemeMode::System`).
    pub theme_mode: crate::theme::ThemeMode,
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
            theme_mode: crate::theme::ThemeMode::System,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        Self::load_with_warning(path).0
    }

    pub fn load_with_warning(path: &Path) -> (Self, Option<String>) {
        match std::fs::read_to_string(path) {
            Ok(raw) => match serde_json::from_str(&raw) {
                Ok(settings) => (settings, None),
                Err(e) => (
                    Self::default(),
                    Some(format!(
                        "Failed to parse settings file {}: {e}",
                        path.display()
                    )),
                ),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Self::default(), None),
            Err(e) => (
                Self::default(),
                Some(format!(
                    "Failed to read settings file {}: {e}",
                    path.display()
                )),
            ),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        persist::save_json_pretty(path, self)
    }
}
