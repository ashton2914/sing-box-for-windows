use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Context;
use chrono::NaiveDateTime;
use eframe::egui;

use crate::config::entry::{ConfigEntry, Source};
use crate::config::settings::{
    InboundOverrideKind, InboundOverrideSettings, LogOverrideSettings, Settings,
    DEFAULT_MIXED_LISTEN, DEFAULT_MIXED_LISTEN_PORT, DEFAULT_TUN_ADDRESS,
    DEFAULT_UPDATE_INTERVAL_HOURS,
};
use crate::config::updater::{self, NewConfigSpec};
use crate::core::paths::Paths;
use crate::core::process::ProcessHandle;
use crate::core::tray::{self, TrayHandle};
use crate::log_bus::LogEvent;

/// Maximum number of log lines retained in the in-memory ring buffer.
/// Older lines are evicted from the front (O(1) on `VecDeque`) once the
/// buffer hits this limit.
const LOG_BACKLOG_CAP: usize = 1000;
const AUTO_UPDATE_FAILURE_RETRY: Duration = Duration::from_secs(10 * 60);

/// Commands sent from the UI thread → background worker.
pub enum BgCmd {
    AddConfig(NewConfigSpec),
    EditConfig {
        slug: String,
        spec: NewConfigSpec,
    },
    UpdateConfig(String),
    DeleteConfig(String),
    SaveSettings(Settings),
    /// Download (if missing) and launch Telerik's AppContainer
    /// Loopback Utility. Network I/O — must run off the UI thread.
    #[cfg(windows)]
    OpenLoopback,
    Shutdown,
}

/// Events sent from background workers → UI thread.
pub enum BgEvent {
    Log(LogEvent),
    AddDone(Result<String, String>),
    EditDone(Result<String, String>),
    UpdateDone(Result<String, String>),
    DeleteDone(Result<String, String>),
    SettingsSaved(Result<(), String>),
    #[cfg(windows)]
    LoopbackDone(Result<(), String>),
}

/// Add-config dialog inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Local,
    Remote,
}

pub struct AddDialogState {
    pub open: bool,
    pub modal_state: crate::theme::ModalState,
    /// `Some(slug)` puts the dialog in edit mode for that entry.
    pub editing_slug: Option<String>,
    pub name: String,
    pub kind: SourceKind,
    pub path: String,
    pub url: String,
    pub busy: bool,
    /// Per-field validation errors. Rendered inline under the matching
    /// input (NOT in the global status banner) so users immediately see
    /// which field needs attention.
    pub name_error: Option<String>,
    pub path_error: Option<String>,
    pub url_error: Option<String>,
}

impl AddDialogState {
    fn new() -> Self {
        Self {
            open: false,
            modal_state: crate::theme::ModalState::default(),
            editing_slug: None,
            name: String::new(),
            kind: SourceKind::Remote,
            path: String::new(),
            url: String::new(),
            busy: false,
            name_error: None,
            path_error: None,
            url_error: None,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Drop any inline validation errors (e.g. when the user edits a
    /// field or switches the source kind, so stale messages don't linger).
    pub fn clear_errors(&mut self) {
        self.name_error = None;
        self.path_error = None;
        self.url_error = None;
    }

    /// Pre-fill the dialog from an existing entry for editing.
    pub fn open_for_edit(&mut self, entry: &ConfigEntry) {
        self.reset();
        self.open = true;
        self.editing_slug = Some(entry.slug.clone());
        self.name = entry.metadata.name.clone();
        match &entry.metadata.source {
            Source::Remote { url } => {
                self.kind = SourceKind::Remote;
                self.url = url.clone();
            }
            Source::Local { path } => {
                self.kind = SourceKind::Local;
                self.path = path.display().to_string();
            }
        }
    }

    pub fn is_edit(&self) -> bool {
        self.editing_slug.is_some()
    }
}

pub struct App {
    pub paths: Paths,
    pub settings: Settings,
    pub proc: Arc<ProcessHandle>,

    pub configs: Vec<ConfigEntry>,
    pub cores: Vec<String>,

    pub logs: VecDeque<LogEvent>,
    /// When `Some`, log rendering is frozen to this snapshot so the
    /// user can read existing lines. Background events keep filling
    /// `logs` normally; resuming drops the snapshot.
    pub log_view_snapshot: Option<Vec<LogEvent>>,
    pub last_error: Option<String>,
    pub last_info: Option<String>,

    /// Cached parsed JSON of the currently-selected config, refreshed
    /// only when the selection slug or file mtime changes. Avoids a
    /// per-frame disk read + JSON parse for the log status toolbar.
    selected_config_cache: SelectedConfigCache,

    /// True while the loopback-utility worker is downloading or
    /// launching the Telerik installer. Drives the Settings button's
    /// disabled state to prevent stacked clicks.
    pub loopback_busy: bool,
    /// Text-edit buffer for the compact update-interval field. The
    /// persisted setting remains numeric; this only preserves in-progress
    /// edits while the field has focus.
    pub update_interval_input: String,

    pub add_dialog: AddDialogState,
    /// `Some(slug)` while the delete confirmation modal is open.
    pub delete_confirm: Option<String>,
    /// Confirm modal for destroying the working directory.
    pub destroy_confirm_open: bool,
    /// About / license dialog open state.
    pub about_open: bool,
    pub about_modal_state: crate::theme::ModalState,
    /// Cached output of `<core> version` for the selected core, keyed by
    /// core name + on-disk mtime so the cache self-invalidates when the
    /// user swaps cores or upgrades the kernel binary.
    pub core_version: CoreVersionCache,
    /// Open state for the modal that surfaces the full multi-line
    /// `version` output (env, tags, revision).
    pub core_version_open: bool,
    pub core_version_modal_state: crate::theme::ModalState,

    pub bg_tx: Sender<BgCmd>,
    pub log_tx: Sender<LogEvent>,
    bg_rx: Receiver<BgEvent>,

    /// Live tray icon, present while `settings.close_to_tray` is on.
    /// Dropping it removes the icon and joins the worker thread.
    tray: Option<TrayHandle>,
    /// HWND of the real eframe root window, captured from `Frame` on the
    /// first update. The tray worker uses this for direct Win32 Show/Hide.
    main_hwnd: Option<usize>,
    /// One-shot startup hide requested by `settings.silent_start`.
    pending_initial_silent_hide: bool,
    /// Cross-thread "is the main window currently mapped" flag. The UI
    /// thread publishes the live `IsWindowVisible` result here at the
    /// top of every `update()`; the log-forwarder thread reads it to
    /// pick a fast vs slow `request_repaint_after` deadline so chatty
    /// sing-box logs don't burn CPU on an invisible UI.
    window_visible: Arc<AtomicBool>,
}

impl App {
    pub fn new(ctx: egui::Context) -> Self {
        let paths = Paths::resolve().expect("Failed to initialize directories");
        let (settings, settings_warning) = Settings::load_with_warning(&paths.settings_file);
        let proc = ProcessHandle::new();

        let (configs, config_load_errors) = paths.list_configs_with_errors();
        let cores = paths.list_cores();
        let pending_initial_silent_hide = settings.silent_start;
        let update_interval_input =
            if settings.update_interval_hours == DEFAULT_UPDATE_INTERVAL_HOURS {
                String::new()
            } else {
                settings.update_interval_hours.to_string()
            };
        let startup_warning = summarize_startup_warnings(settings_warning, &config_load_errors);

        let (bg_tx, bg_cmd_rx) = channel::<BgCmd>();
        let (bg_event_tx, bg_rx) = channel::<BgEvent>();
        let (log_tx, log_rx) = channel::<LogEvent>();

        let window_visible = Arc::new(AtomicBool::new(true));

        // Forward LogEvents into the unified UI event channel.
        //
        // sing-box is chatty even at info level (DNS, connection
        // tracking, periodic stats). The old code called
        // `ctx.request_repaint()` per line, which turned every log line
        // into a full layout pass — the dominant idle-CPU cost while
        // the window was hidden to the tray.
        //
        // Strategy: use `request_repaint_after` with a debounce window.
        // Multiple calls within the window collapse to a single
        // scheduled repaint (egui keeps the earliest pending deadline),
        // so a burst of N log lines costs ONE eventual `update()` that
        // drains all of them out of the channel together. When the
        // window is hidden, we stretch the window way out so the UI
        // thread is barely woken at all.
        {
            let bg_event_tx = bg_event_tx.clone();
            let ctx = ctx.clone();
            let window_visible = window_visible.clone();
            thread::spawn(move || {
                // 50ms ≈ 20 Hz refresh during log bursts — fast enough
                // for the live Logs card to feel real-time, slow enough
                // to coalesce dozens of lines per paint.
                const VISIBLE_REPAINT: Duration = Duration::from_millis(50);
                // While hidden the user can't see anything, but we
                // still need an occasional drain so the channel and
                // the in-memory log ring buffer don't grow without
                // bound. 2s keeps both at trivially bounded sizes
                // while practically eliminating idle CPU.
                const HIDDEN_REPAINT: Duration = Duration::from_secs(2);
                while let Ok(ev) = log_rx.recv() {
                    if bg_event_tx.send(BgEvent::Log(ev)).is_err() {
                        break;
                    }
                    let delay = if window_visible.load(Ordering::Relaxed) {
                        VISIBLE_REPAINT
                    } else {
                        HIDDEN_REPAINT
                    };
                    ctx.request_repaint_after(delay);
                }
            });
        }

        // Background worker.
        {
            let paths = paths.clone();
            let settings = settings.clone();
            let log_tx = log_tx.clone();
            let bg_event_tx = bg_event_tx.clone();
            let ctx = ctx.clone();
            thread::spawn(move || {
                background_loop(paths, settings, bg_cmd_rx, log_tx, bg_event_tx, ctx);
            });
        }

        let mut app = Self {
            paths,
            settings,
            proc,
            configs,
            cores,
            logs: VecDeque::with_capacity(LOG_BACKLOG_CAP),
            log_view_snapshot: None,
            last_error: startup_warning,
            last_info: None,
            selected_config_cache: SelectedConfigCache::default(),
            loopback_busy: false,
            update_interval_input,
            add_dialog: AddDialogState::new(),
            delete_confirm: None,
            destroy_confirm_open: false,
            about_open: false,
            about_modal_state: crate::theme::ModalState::default(),
            core_version: CoreVersionCache::default(),
            core_version_open: false,
            core_version_modal_state: crate::theme::ModalState::default(),
            bg_tx,
            log_tx,
            bg_rx,
            tray: None,
            main_hwnd: None,
            pending_initial_silent_hide,
            window_visible,
        };

        // Drop a stale selection (config or core) if its file/folder is
        // gone — reuses the same pruning logic that the Settings refresh
        // button triggers so startup and runtime behaviour stay aligned.
        app.prune_stale_selections();

        if app.settings.auto_start_sing_box {
            app.try_start();
        }

        // One-shot cleanup for HKCU\\\u2026\\Run values and Task Scheduler
        // tasks left behind by previously-deprecated launcher names. Has
        // to run BEFORE `sync_persistent_state` so the rewrite below
        // doesn't race with a stale legacy entry pointing at an old exe.
        crate::core::autostart::migrate_legacy();
        crate::core::elevation::migrate_legacy_admin_task();

        // Reconcile the Windows autostart entry and the persistent-admin
        // scheduled task with the freshly loaded settings. Idempotent —
        // every launch self-heals if the user moved the exe, deleted the
        // task externally, etc.
        app.sync_persistent_state();
        app.sync_tray();
        app
    }

    pub fn refresh_listings(&mut self) -> Option<String> {
        let (configs, config_load_errors) = self.paths.list_configs_with_errors();
        self.configs = configs;
        self.cores = self.paths.list_cores();
        // If the previously-selected config or core has disappeared
        // from disk (deleted file/folder, renamed, etc.), drop the
        // stale selection so the dropdowns no longer surface a name
        // that resolves to nothing.
        self.prune_stale_selections();
        summarize_config_load_errors(&config_load_errors)
    }

    /// Clear `selected_config` and/or `selected_core` when the underlying
    /// entry is no longer present in the freshly-listed `configs` /
    /// `cores` collections, then persist if anything changed. Shared by
    /// `App::new` (startup self-heal) and `refresh_listings` (Settings
    /// refresh button).
    fn prune_stale_selections(&mut self) {
        let mut changed = false;
        if let Some(sel) = self.settings.selected_config.clone() {
            if !self.configs.iter().any(|c| c.slug == sel) {
                self.settings.selected_config = None;
                changed = true;
            }
        }
        if let Some(sel) = self.settings.selected_core.clone() {
            if !self.cores.iter().any(|c| c == &sel) {
                self.settings.selected_core = None;
                // Invalidate the cached version info so the toolbar
                // chip stops showing a version that no longer maps to
                // an installed core.
                self.core_version = CoreVersionCache::default();
                changed = true;
            }
        }
        if changed {
            self.persist_settings();
        }
    }

    /// Reconcile the on-disk Windows autostart entry and the Task
    /// Scheduler "always run as administrator" task with the current
    /// `Settings`. Called from both `App::new` (startup self-heal) and
    /// `persist_settings` (after every UI mutation).
    ///
    /// Coupling rules — chosen to match the PowerToys autostart pattern:
    /// * `always_admin` ON  + `auto_start` ON → task with LogonTrigger
    ///   handles the auto-launch; the `HKCU\...\Run` key is removed
    ///   (avoids dual launch).
    /// * `always_admin` ON  + `auto_start` OFF → on-demand task only;
    ///   user-launched std-user instance silently promotes via
    ///   `schtasks /run`.
    /// * `always_admin` OFF + `auto_start` ON → no task; Run key is set.
    /// * `always_admin` OFF + `auto_start` OFF → no task; no Run key.
    fn sync_persistent_state(&mut self) {
        use crate::core::{autostart, elevation};

        if self.settings.always_admin {
            if elevation::is_elevated() {
                if let Err(e) =
                    elevation::ensure_admin_task(&self.paths.root, self.settings.auto_start)
                {
                    self.last_error =
                        Some(format!("Failed to register persistent admin task: {e}"));
                }
            }
            // Remove the Run key whether or not the task registration
            // succeeded — when always_admin is on, the Run key never
            // makes sense (it'd launch a non-admin sibling).
            if let Err(e) = autostart::set_enabled(false) {
                self.last_error = Some(format!("Failed to clear Windows autostart entry: {e}"));
            }
        } else {
            if let Err(e) = elevation::delete_admin_task() {
                self.last_error = Some(format!("Failed to remove persistent admin task: {e}"));
            }
            if let Err(e) = autostart::set_enabled(self.settings.auto_start) {
                self.last_error = Some(format!("Failed to update Windows autostart entry: {e}"));
            }
        }
    }

    pub fn persist_settings(&mut self) {
        self.sync_persistent_state();
        self.sync_tray();
        let _ = self.bg_tx.send(BgCmd::SaveSettings(self.settings.clone()));
    }

    /// Bring the tray icon in line with `settings.close_to_tray`. Spawns
    /// the worker thread on toggle-on; drops the handle (which removes
    /// the icon and joins the worker) on toggle-off.
    fn sync_tray(&mut self) {
        if self.settings.close_to_tray || self.settings.silent_start {
            if self.tray.is_none() {
                match tray::spawn(self.proc.clone(), self.log_tx.clone()) {
                    Some(handle) => {
                        if let Some(hwnd) = self.main_hwnd {
                            handle.set_main_hwnd(hwnd);
                        }
                        self.tray = Some(handle);
                    }
                    None => {
                        self.last_error = Some("Failed to create system tray icon.".into());
                    }
                }
            }
        } else if self.tray.is_some() {
            // Dropping the handle posts WM_CLOSE to the tray worker,
            // which removes the icon and joins the worker thread.
            self.tray = None;
        }
    }

    pub fn selected_entry(&self) -> Option<&ConfigEntry> {
        let sel = self.settings.selected_config.as_deref()?;
        self.configs.iter().find(|c| c.slug == sel)
    }

    /// Return the parsed JSON of the currently-selected config, reading
    /// from disk only when the selection or the file's mtime changes.
    /// Returns `None` if no config is selected or the file is missing or
    /// not valid JSON.
    pub fn selected_config_value(&mut self) -> Option<&serde_json::Value> {
        let entry = self.selected_entry()?;
        let slug = entry.slug.clone();
        let path = entry.config_file();
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        self.selected_config_cache
            .get_or_refresh(slug, &path, mtime)
    }

    /// Refresh `self.core_version` from `<core> version` if the cached
    /// entry no longer matches the currently selected core (or its
    /// on-disk mtime). Synchronous because the command is short-lived
    /// and only runs on a cache miss, not every frame.
    pub fn ensure_core_version(&mut self) {
        // Rate-limit the metadata() probe so idle repaints don't fan
        // out into one stat syscall per frame. A 1-second debounce is
        // imperceptible for detecting kernel swaps while keeping the
        // common cache-hit path effectively free.
        let now = Instant::now();
        if let Some(last) = self.core_version.last_check {
            if now.duration_since(last) < Duration::from_secs(1) {
                return;
            }
        }
        self.core_version.last_check = Some(now);

        let Some(name) = self.settings.selected_core.clone() else {
            // No core selected — keep the cache cleared so the UI shows
            // a neutral placeholder instead of stale version text.
            if self.core_version.name.is_some() {
                self.core_version = CoreVersionCache {
                    last_check: Some(now),
                    ..CoreVersionCache::default()
                };
            }
            return;
        };
        let path = self.paths.core_path(&name);
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if self.core_version.name.as_deref() == Some(name.as_str())
            && self.core_version.mtime == mtime
        {
            return;
        }
        let (full, short) = match crate::core::process::fetch_core_version(&path) {
            Ok(text) => {
                let short = extract_short_version(&text);
                (Some(text), short)
            }
            // Cache the failed attempt (with cleared full/short) so we
            // don't spawn the process every frame; the cache still
            // refreshes when the user selects a different core or
            // replaces the binary on disk.
            Err(_) => (None, None),
        };
        self.core_version = CoreVersionCache {
            name: Some(name),
            mtime,
            full,
            short,
            last_check: Some(now),
        };
    }

    pub fn try_start(&mut self) {
        let Some(core_name) = self.settings.selected_core.clone() else {
            self.last_error = Some("No sing-box core selected".into());
            return;
        };
        let Some(entry) = self.selected_entry().cloned() else {
            self.last_error = Some("No config selected".into());
            return;
        };
        let core_exe = self.paths.core_path(&core_name);
        let cfg_path = entry.config_file();
        let runtime_cfg = self.paths.runtime_config_file();
        if let Err(e) = prepare_runtime_config(&cfg_path, &runtime_cfg, &self.settings) {
            self.last_error = Some(e.to_string());
            return;
        }
        match self.proc.start(
            &core_exe,
            &runtime_cfg,
            &self.paths.working_dir,
            self.log_tx.clone(),
        ) {
            Ok(()) => {
                self.last_error = None;
                self.last_info = None;
            }
            Err(e) => self.last_error = Some(e.to_string()),
        }
    }

    pub fn try_stop(&mut self) {
        let _ = self.proc.stop(self.log_tx.clone());
    }

    /// Validate the dialog inputs and dispatch (add or edit) to the worker.
    pub fn submit_add_dialog(&mut self) {
        // Reset inline errors and validate every field up-front so the
        // user sees ALL problems at once instead of one-at-a-time.
        self.add_dialog.clear_errors();

        let name = self.add_dialog.name.trim().to_string();
        if name.is_empty() {
            self.add_dialog.name_error = Some("Name is required".into());
        }

        let source = match self.add_dialog.kind {
            SourceKind::Local => {
                let path = self.add_dialog.path.trim();
                if path.is_empty() {
                    self.add_dialog.path_error = Some("File path is required".into());
                    None
                } else {
                    Some(Source::Local {
                        path: PathBuf::from(path),
                    })
                }
            }
            SourceKind::Remote => {
                let url = self.add_dialog.url.trim();
                if url.is_empty() {
                    self.add_dialog.url_error = Some("URL is required".into());
                    None
                } else {
                    Some(Source::Remote {
                        url: url.to_string(),
                    })
                }
            }
        };

        // Bail out if any field failed validation. Errors are rendered
        // inline next to their inputs by the modal, so we deliberately
        // do NOT touch `self.last_error` here.
        let Some(source) = source else { return };
        if self.add_dialog.name_error.is_some() {
            return;
        }

        self.add_dialog.busy = true;
        self.last_error = None;
        let spec = NewConfigSpec { name, source };
        match self.add_dialog.editing_slug.clone() {
            Some(slug) => {
                let _ = self.bg_tx.send(BgCmd::EditConfig { slug, spec });
            }
            None => {
                let _ = self.bg_tx.send(BgCmd::AddConfig(spec));
            }
        }
    }

    fn drain_events(&mut self) {
        // Surface any unexpected sing-box exit (config error, panic,
        // missing TUN privileges, …) in the red banner. The watcher
        // thread populates this slot whenever the child terminates
        // without a `stop()` call having marked the exit as expected.
        if let Some(msg) = self.proc.take_exit_error() {
            self.last_error = Some(msg);
            self.last_info = None;
        }

        while let Ok(ev) = self.bg_rx.try_recv() {
            match ev {
                BgEvent::Log(le) => {
                    if self.logs.len() == LOG_BACKLOG_CAP {
                        self.logs.pop_front();
                    }
                    self.logs.push_back(le);
                }
                BgEvent::AddDone(Ok(slug)) => {
                    self.add_dialog.busy = false;
                    self.add_dialog.reset();
                    let listing_warning = self.refresh_listings();
                    self.settings.selected_config = Some(slug.clone());
                    self.persist_settings();
                    self.last_error = listing_warning;
                    self.last_info = self.last_error.is_none().then(|| format!("Added '{slug}'"));
                }
                BgEvent::AddDone(Err(e)) => {
                    self.add_dialog.busy = false;
                    self.last_error = Some(e);
                }
                BgEvent::EditDone(Ok(slug)) => {
                    self.add_dialog.busy = false;
                    self.add_dialog.reset();
                    let listing_warning = self.refresh_listings();
                    self.last_error = listing_warning;
                    self.last_info = self
                        .last_error
                        .is_none()
                        .then(|| format!("Updated '{slug}'"));
                }
                BgEvent::EditDone(Err(e)) => {
                    self.add_dialog.busy = false;
                    self.last_error = Some(e);
                }
                BgEvent::UpdateDone(Ok(slug)) => {
                    let listing_warning = self.refresh_listings();
                    self.last_error = listing_warning;
                    self.last_info = self
                        .last_error
                        .is_none()
                        .then(|| format!("Updated '{slug}'"));
                }
                BgEvent::UpdateDone(Err(e)) => {
                    self.last_error = Some(e);
                }
                BgEvent::DeleteDone(Ok(slug)) => {
                    if self.settings.selected_config.as_deref() == Some(&slug) {
                        self.settings.selected_config = None;
                        self.persist_settings();
                    }
                    let listing_warning = self.refresh_listings();
                    self.last_error = listing_warning;
                    self.last_info = self
                        .last_error
                        .is_none()
                        .then(|| format!("Deleted '{slug}'"));
                }
                BgEvent::DeleteDone(Err(e)) => {
                    self.last_error = Some(e);
                }
                BgEvent::SettingsSaved(Ok(())) => {}
                BgEvent::SettingsSaved(Err(e)) => {
                    self.last_error = Some(format!("Failed to save settings: {e}"));
                }
                #[cfg(windows)]
                BgEvent::LoopbackDone(res) => {
                    self.loopback_busy = false;
                    match res {
                        Ok(()) => {
                            self.last_error = None;
                            self.last_info =
                                Some(format!("Launched {}", crate::core::loopback::DISPLAY_NAME));
                        }
                        Err(e) => {
                            self.last_error = Some(format!("Loopback utility failed: {e}"));
                        }
                    }
                }
            }
        }
    }

    /// Intercept the OS-level close (X button / Alt+F4): when the
    /// "Close button hides to tray" setting is on AND the tray worker
    /// is healthy, cancel the close and hide the real Win32 window
    /// directly. The tray worker later shows that HWND directly too, so
    /// this no longer depends on hidden-window eframe updates.
    fn handle_close_request(&mut self, ctx: &egui::Context) {
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested && self.settings.close_to_tray && self.tray.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if let (Some(tray), Some(hwnd)) = (self.tray.as_ref(), self.main_hwnd) {
                tray.hide_main_window(hwnd);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }
    }

    fn apply_initial_silent_hide(&mut self, ctx: &egui::Context) {
        if !self.pending_initial_silent_hide || self.main_hwnd.is_none() {
            return;
        }
        self.pending_initial_silent_hide = false;

        if let (Some(tray), Some(hwnd)) = (self.tray.as_ref(), self.main_hwnd) {
            tray.hide_main_window(hwnd);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        }
    }

    /// Returns `true` while the real Win32 main window is mapped / not
    /// hidden to the tray. Used as a gate before scheduling periodic
    /// `request_repaint_after` ticks (e.g. the running-uptime refresh):
    /// when the window is hidden, those ticks would still wake the UI
    /// thread and force a full layout pass that nobody can see, which
    /// is the dominant idle-CPU cost while sitting in the tray.
    ///
    /// Reads from `window_visible`, which `update()` refreshes from
    /// `IsWindowVisible` once per frame. Cheaper than re-querying Win32
    /// from every UI widget that wants to know, and consistent with the
    /// value the log-forwarder thread sees.
    pub fn main_window_visible(&self) -> bool {
        self.window_visible.load(Ordering::Relaxed)
    }

    /// Live Win32 query — the source of truth that feeds the cached
    /// `window_visible` atomic. Conservatively returns `true` before the
    /// HWND is captured so we never suppress the first paint.
    fn is_main_window_visible(&self) -> bool {
        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;

            match self.main_hwnd {
                Some(hwnd) => unsafe { IsWindowVisible(hwnd as _) != 0 },
                None => true,
            }
        }
        #[cfg(not(windows))]
        {
            true
        }
    }
}

fn summarize_startup_warnings(
    settings_warning: Option<String>,
    config_errors: &[String],
) -> Option<String> {
    let mut warnings = Vec::new();
    if let Some(warning) = settings_warning {
        warnings.push(warning);
    }
    if let Some(warning) = summarize_config_load_errors(config_errors) {
        warnings.push(warning);
    }
    (!warnings.is_empty()).then(|| warnings.join("\n"))
}

fn summarize_config_load_errors(errors: &[String]) -> Option<String> {
    match errors {
        [] => None,
        [one] => Some(one.clone()),
        many => Some(format!(
            "Failed to load {} config entries. First error: {}",
            many.len(),
            many[0]
        )),
    }
}

fn prepare_runtime_config(
    source: &std::path::Path,
    runtime: &std::path::Path,
    settings: &Settings,
) -> anyhow::Result<()> {
    if !settings.inbound_override.enabled && !settings.log_override.enabled {
        if let Some(parent) = runtime.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create runtime config directory {}",
                    parent.display()
                )
            })?;
        }
        std::fs::copy(source, runtime).with_context(|| {
            format!(
                "failed to copy config {} to {}",
                source.display(),
                runtime.display()
            )
        })?;
        return Ok(());
    }

    let raw = std::fs::read(source)
        .with_context(|| format!("failed to read config {}", source.display()))?;
    let mut config: serde_json::Value = serde_json::from_slice(&raw)
        .with_context(|| format!("failed to parse config JSON {}", source.display()))?;
    let Some(root) = config.as_object_mut() else {
        anyhow::bail!("config root must be a JSON object: {}", source.display());
    };

    if settings.inbound_override.enabled {
        root.insert(
            "inbounds".to_owned(),
            serde_json::Value::Array(vec![build_override_inbound(&settings.inbound_override)?]),
        );
    }
    if settings.log_override.enabled {
        root.insert("log".to_owned(), build_override_log(&settings.log_override));
    }

    let bytes = serde_json::to_vec_pretty(&config)
        .with_context(|| format!("failed to serialize runtime config {}", runtime.display()))?;
    if let Some(parent) = runtime.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create runtime config directory {}",
                parent.display()
            )
        })?;
    }
    std::fs::write(runtime, bytes)
        .with_context(|| format!("failed to write runtime config {}", runtime.display()))?;
    Ok(())
}

fn build_override_inbound(settings: &InboundOverrideSettings) -> anyhow::Result<serde_json::Value> {
    match settings.kind {
        InboundOverrideKind::MixedIn => {
            let listen = non_empty_or_default(&settings.mixed_listen, DEFAULT_MIXED_LISTEN);
            let port = parse_optional_port(
                &settings.mixed_listen_port,
                DEFAULT_MIXED_LISTEN_PORT,
                "mixed listen port",
            )?;
            Ok(serde_json::json!({
                "type": "mixed",
                "tag": "mixed-in",
                "listen": listen,
                "listen_port": port,
            }))
        }
        InboundOverrideKind::Tun => {
            let mut inbound = serde_json::json!({
                "type": "tun",
                "tag": "tun-in",
                "address": DEFAULT_TUN_ADDRESS,
                "auto_route": true,
                "strict_route": true,
                "stack": "system",
                "endpoint_independent_nat": settings.tun_endpoint_independent_nat,
            });
            let mtu = settings.tun_mtu.trim();
            if !mtu.is_empty() {
                let mtu: u64 = mtu
                    .parse()
                    .with_context(|| format!("invalid TUN MTU value: {mtu}"))?;
                inbound["mtu"] = serde_json::Value::Number(mtu.into());
            }
            Ok(inbound)
        }
    }
}

#[derive(Default)]
pub struct CoreVersionCache {
    /// Name of the core executable whose version we cached. `None`
    /// means we have never run the lookup successfully for any core.
    name: Option<String>,
    /// On-disk mtime captured at fetch time so an in-place kernel
    /// upgrade invalidates the cache automatically.
    mtime: Option<std::time::SystemTime>,
    /// Full multi-line stdout from `<core> version`. `None` when the
    /// lookup failed (the cache still records the attempt so we don't
    /// re-spawn the process every frame).
    pub full: Option<String>,
    /// Short version token extracted from the first line of `full`
    /// (e.g. `1.10.0`). `None` when extraction failed or the lookup
    /// failed.
    pub short: Option<String>,
    /// Wall-clock time of the most recent `metadata()` probe. Used to
    /// rate-limit the per-frame stat call so idle 60fps repaints do not
    /// fan out into 60 syscalls/second on the core exe.
    last_check: Option<Instant>,
}

/// Pull a compact version token out of the first line of `<core> version`
/// output. sing-box prints `sing-box version 1.10.0` as the first line;
/// we take the last whitespace-separated token and assume that is the
/// version number. Falls back to the trimmed first line if no whitespace
/// is present.
fn extract_short_version(full: &str) -> Option<String> {
    let first = full.lines().next()?.trim();
    if first.is_empty() {
        return None;
    }
    let token = first.split_whitespace().last().unwrap_or(first);
    Some(token.to_string())
}

#[derive(Default)]
struct SelectedConfigCache {
    slug: Option<String>,
    mtime: Option<std::time::SystemTime>,
    value: Option<serde_json::Value>,
}

impl SelectedConfigCache {
    /// Return the parsed JSON for `slug` at `path`, refreshing the
    /// cached value when either the slug or `mtime` differs from the
    /// last call. `mtime` is what `App` already read via `metadata` to
    /// avoid a second stat syscall here.
    fn get_or_refresh(
        &mut self,
        slug: String,
        path: &std::path::Path,
        mtime: Option<std::time::SystemTime>,
    ) -> Option<&serde_json::Value> {
        let same = self.slug.as_deref() == Some(slug.as_str()) && self.mtime == mtime;
        if !same {
            self.slug = Some(slug);
            self.mtime = mtime;
            self.value = std::fs::read_to_string(path)
                .ok()
                .and_then(|raw| serde_json::from_str(&raw).ok());
        }
        self.value.as_ref()
    }
}

fn build_override_log(settings: &LogOverrideSettings) -> serde_json::Value {
    let mut log = serde_json::Map::new();
    log.insert(
        "disabled".to_owned(),
        serde_json::Value::Bool(settings.disabled),
    );
    log.insert(
        "level".to_owned(),
        serde_json::Value::String(settings.level.as_str().to_owned()),
    );
    if settings.save_logs {
        log.insert(
            "output".to_owned(),
            serde_json::Value::String("box.log".to_owned()),
        );
    }
    log.insert("timestamp".to_owned(), serde_json::Value::Bool(true));
    serde_json::Value::Object(log)
}

fn non_empty_or_default<'a>(value: &'a str, default: &'static str) -> &'a str {
    let value = value.trim();
    if value.is_empty() {
        default
    } else {
        value
    }
}

fn parse_optional_port(value: &str, default: u16, label: &str) -> anyhow::Result<u16> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(default);
    }
    value
        .parse::<u16>()
        .with_context(|| format!("invalid {label}: {value}"))
        .and_then(|port| {
            if port == 0 {
                anyhow::bail!("invalid {label}: {value}");
            }
            Ok(port)
        })
}

#[cfg(windows)]
fn main_hwnd_from_frame(frame: &eframe::Frame) -> Option<usize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    match frame.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as usize),
        _ => None,
    }
}

#[cfg(not(windows))]
fn main_hwnd_from_frame(_frame: &eframe::Frame) -> Option<usize> {
    None
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        crate::theme::color::surface().to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Must run BEFORE any widget consumes input. Drops Enter while
        // an IME composition is active so a TextEdit doesn't surrender
        // focus mid-commit and accidentally bake the preedit (e.g. raw
        // pinyin "de'ji'd'j'e") into its buffer.
        crate::theme::swallow_enter_during_ime(ctx);

        // Reconcile the active palette against the user's `theme_mode`
        // preference + the current OS theme. Cheap (one atomic load
        // and a struct-eq); only re-applies the egui Style when the
        // resolved is_dark flag changed.
        {
            let system_is_dark = match frame.info().system_theme {
                Some(eframe::Theme::Dark) => true,
                Some(eframe::Theme::Light) => false,
                None => true,
            };
            let want_dark = self.settings.theme_mode.resolve(system_is_dark);
            if crate::theme::is_dark() != want_dark {
                crate::theme::set_dark(want_dark);
                crate::theme::apply(ctx);
                ctx.request_repaint();
            }
        }

        if self.main_hwnd.is_none() {
            self.main_hwnd = main_hwnd_from_frame(frame);
            if let (Some(tray), Some(hwnd)) = (self.tray.as_ref(), self.main_hwnd) {
                tray.set_main_hwnd(hwnd);
            }
        }

        // Refresh the cross-thread visibility flag every frame so the
        // log forwarder picks the right repaint cadence. Done after
        // the HWND has been captured (otherwise the Win32 query
        // pessimistically falls back to "visible" and the flag would
        // never go false the first time we hide).
        self.window_visible
            .store(self.is_main_window_visible(), Ordering::Relaxed);

        self.apply_initial_silent_hide(ctx);
        self.drain_events();
        self.handle_close_request(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            crate::ui::show(ui, self);
        });

        // No unconditional periodic repaint:
        //   * the log forwarder, the bg worker, the process exit watcher
        //     and the IME swallower all already call `ctx.request_repaint()`
        //     when they have something to show.
        //   * with the 750ms timer the app would burn one full layout pass
        //     per ~1.3s while completely idle, even with the window hidden
        //     to the tray.
        // The (rare) status→stopped transition is still picked up the next
        // frame after `take_exit_error` posts an error, or as soon as the
        // user moves the cursor over the window.
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.bg_tx.send(BgCmd::Shutdown);
        let _ = self.proc.stop(self.log_tx.clone());
    }
}

fn background_loop(
    paths: Paths,
    mut settings: Settings,
    cmd_rx: Receiver<BgCmd>,
    log_tx: Sender<LogEvent>,
    event_tx: Sender<BgEvent>,
    ctx: egui::Context,
) {
    let mut last_auto_attempt: Option<(String, Instant)> = None;

    loop {
        let timeout = Duration::from_secs(30);
        match cmd_rx.recv_timeout(timeout) {
            Ok(BgCmd::AddConfig(spec)) => {
                let res = updater::add_config(&paths, &spec, &log_tx).map_err(|e| e.to_string());
                let _ = event_tx.send(BgEvent::AddDone(res));
                ctx.request_repaint();
            }
            Ok(BgCmd::EditConfig { slug, spec }) => {
                let res = run_edit(&paths, &slug, &spec, &log_tx);
                let _ = event_tx.send(BgEvent::EditDone(res));
                ctx.request_repaint();
            }
            Ok(BgCmd::UpdateConfig(slug)) => {
                let res = run_update_one(&paths, &slug, &log_tx);
                let _ = event_tx.send(BgEvent::UpdateDone(res));
                ctx.request_repaint();
            }
            Ok(BgCmd::DeleteConfig(slug)) => {
                let res = run_delete(&paths, &slug, &log_tx);
                let _ = event_tx.send(BgEvent::DeleteDone(res));
                ctx.request_repaint();
            }
            Ok(BgCmd::SaveSettings(new_settings)) => {
                settings = new_settings;
                let res = settings
                    .save(&paths.settings_file)
                    .map_err(|e| e.to_string());
                let _ = event_tx.send(BgEvent::SettingsSaved(res));
                ctx.request_repaint();
            }
            #[cfg(windows)]
            Ok(BgCmd::OpenLoopback) => {
                let res =
                    crate::core::loopback::open_or_download(&paths).map_err(|e| e.to_string());
                let _ = event_tx.send(BgEvent::LoopbackDone(res));
                ctx.request_repaint();
            }
            Ok(BgCmd::Shutdown) => break,
            Err(RecvTimeoutError::Timeout) => {
                if settings.auto_update {
                    if let Some(slug) = settings.selected_config.clone() {
                        let interval = Duration::from_secs(
                            settings.update_interval_hours.max(1).saturating_mul(3600),
                        );
                        // Only auto-update remote entries. The persisted
                        // `last_updated` timestamp is the source of truth so
                        // restart/uptime quirks do not reset the schedule.
                        if let Some(entry) = paths.load_entry(&slug) {
                            if matches!(entry.metadata.source, Source::Remote { .. })
                                && should_auto_update(&entry, interval)
                                && can_attempt_auto_update(&last_auto_attempt, &slug)
                            {
                                last_auto_attempt = Some((slug.clone(), Instant::now()));
                                let res = updater::refresh_entry(&entry, &log_tx)
                                    .map(|_| slug.clone())
                                    .map_err(|e| e.to_string());
                                let _ = event_tx.send(BgEvent::UpdateDone(res));
                                ctx.request_repaint();
                            }
                        }
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn can_attempt_auto_update(last_attempt: &Option<(String, Instant)>, slug: &str) -> bool {
    match last_attempt {
        Some((last_slug, last_at)) if last_slug == slug => {
            last_at.elapsed() >= AUTO_UPDATE_FAILURE_RETRY
        }
        _ => true,
    }
}

fn should_auto_update(entry: &ConfigEntry, interval: Duration) -> bool {
    let Some(raw) = entry.metadata.last_updated.as_deref() else {
        return true;
    };

    let Ok(last_updated) = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S") else {
        return true;
    };

    let now = chrono::Local::now().naive_local();
    if last_updated > now {
        return true;
    }

    now.signed_duration_since(last_updated)
        .to_std()
        .map(|elapsed| elapsed >= interval)
        .unwrap_or(true)
}

/// Look up a single `ConfigEntry` by slug, or return a uniform
/// "not found" error string. Centralised here so the `update`, `edit`
/// and `delete` paths all share the same lookup + error wording.
fn lookup_entry(paths: &Paths, slug: &str) -> Result<ConfigEntry, String> {
    paths
        .load_entry(slug)
        .ok_or_else(|| format!("config '{slug}' not found"))
}

fn run_update_one(paths: &Paths, slug: &str, log_tx: &Sender<LogEvent>) -> Result<String, String> {
    let entry = lookup_entry(paths, slug)?;
    updater::refresh_entry(&entry, log_tx)
        .map(|_| slug.to_string())
        .map_err(|e| e.to_string())
}

fn run_edit(
    paths: &Paths,
    slug: &str,
    spec: &NewConfigSpec,
    log_tx: &Sender<LogEvent>,
) -> Result<String, String> {
    let entry = lookup_entry(paths, slug)?;
    updater::edit_entry(&entry, &spec.name, &spec.source, log_tx)
        .map(|_| slug.to_string())
        .map_err(|e| e.to_string())
}

fn run_delete(paths: &Paths, slug: &str, log_tx: &Sender<LogEvent>) -> Result<String, String> {
    let entry = lookup_entry(paths, slug)?;
    updater::delete_entry(&entry, log_tx)
        .map(|_| slug.to_string())
        .map_err(|e| e.to_string())
}
