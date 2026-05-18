use std::path::Path;

use serde::{de, Deserialize, Deserializer, Serialize};

use crate::config::persist;

pub const DEFAULT_MIXED_LISTEN: &str = "127.0.0.1";
pub const DEFAULT_MIXED_LISTEN_PORT: u16 = 5353;
pub const DEFAULT_TUN_ADDRESS: &str = "172.18.0.1/30";
pub const DEFAULT_UPDATE_INTERVAL_HOURS: u64 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum InboundOverrideKind {
    #[default]
    MixedIn,
    Tun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
    Fatal,
    Panic,
}

impl LogLevel {
    /// sing-box log level name, also used as the UI label.
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
            LogLevel::Fatal => "fatal",
            LogLevel::Panic => "panic",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct LogOverrideSettings {
    pub enabled: bool,
    pub disabled: bool,
    pub level: LogLevel,
    pub save_logs: bool,
}

impl Default for LogOverrideSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            disabled: false,
            level: LogLevel::Info,
            save_logs: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct InboundOverrideSettings {
    pub enabled: bool,
    pub kind: InboundOverrideKind,
    pub mixed_listen: String,
    #[serde(default, deserialize_with = "string_from_string_or_number")]
    pub mixed_listen_port: String,
    pub tun_mtu: String,
    pub tun_endpoint_independent_nat: bool,
}

impl Default for InboundOverrideSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: InboundOverrideKind::MixedIn,
            mixed_listen: String::new(),
            mixed_listen_port: String::new(),
            tun_mtu: String::new(),
            tun_endpoint_independent_nat: true,
        }
    }
}

impl InboundOverrideSettings {
    fn normalize_empty_defaults(&mut self) {
        if self.mixed_listen.trim() == DEFAULT_MIXED_LISTEN {
            self.mixed_listen.clear();
        }
        if self.mixed_listen_port.trim() == DEFAULT_MIXED_LISTEN_PORT.to_string() {
            self.mixed_listen_port.clear();
        }
    }
}

fn string_from_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Null => Ok(String::new()),
        serde_json::Value::String(value) => Ok(value),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        other => Err(de::Error::custom(format!(
            "expected string or number for mixed_listen_port, got {other}"
        ))),
    }
}

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
    /// Optional runtime-only sing-box inbound override. The original
    /// selected config is never modified; the launcher writes a copied
    /// runtime config under the sing-box working directory before start.
    pub inbound_override: InboundOverrideSettings,
    /// Optional runtime-only sing-box log override. Fixed fields like
    /// `timestamp` and the output path are supplied by the launcher.
    pub log_override: LogOverrideSettings,
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
            update_interval_hours: DEFAULT_UPDATE_INTERVAL_HOURS,
            close_to_tray: false,
            silent_start: false,
            theme_mode: crate::theme::ThemeMode::System,
            inbound_override: InboundOverrideSettings::default(),
            log_override: LogOverrideSettings::default(),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        Self::load_with_warning(path).0
    }

    pub fn load_with_warning(path: &Path) -> (Self, Option<String>) {
        match std::fs::read_to_string(path) {
            Ok(raw) => match serde_json::from_str::<Self>(&raw) {
                Ok(mut settings) => {
                    settings.inbound_override.normalize_empty_defaults();
                    (settings, None)
                }
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
