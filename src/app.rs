use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::config::entry::{ConfigEntry, Source};
use crate::config::settings::Settings;
use crate::config::updater::{self, NewConfigSpec};
use crate::core::paths::Paths;
use crate::core::process::ProcessHandle;
use crate::core::tray::{self, TrayHandle};
use crate::log_bus::LogEvent;

/// Maximum number of log lines retained in the in-memory ring buffer.
/// Older lines are evicted from the front (O(1) on `VecDeque`) once the
/// buffer hits this limit.
const LOG_BACKLOG_CAP: usize = 1000;

/// Commands sent from the UI thread → background worker.
pub enum BgCmd {
    AddConfig(NewConfigSpec),
    EditConfig { slug: String, spec: NewConfigSpec },
    UpdateConfig(String),
    DeleteConfig(String),
    SaveSettings(Settings),
    Shutdown,
}

/// Events sent from background workers → UI thread.
pub enum BgEvent {
    Log(LogEvent),
    AddDone(Result<String, String>),
    EditDone(Result<String, String>),
    UpdateDone(Result<String, String>),
    DeleteDone(Result<String, String>),
    SettingsSaved,
}

/// Add-config dialog inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Local,
    Remote,
}

pub struct AddDialogState {
    pub open: bool,
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
    pub last_error: Option<String>,
    pub last_info: Option<String>,

    pub add_dialog: AddDialogState,
    /// `Some(slug)` while the delete confirmation modal is open.
    pub delete_confirm: Option<String>,
    /// Confirm modal for destroying the working directory.
    pub destroy_confirm_open: bool,

    pub bg_tx: Sender<BgCmd>,
    pub log_tx: Sender<LogEvent>,
    bg_rx: Receiver<BgEvent>,

    /// Live tray icon, present while `settings.close_to_tray` is on.
    /// Dropping it removes the icon and joins the worker thread.
    tray: Option<TrayHandle>,
    /// HWND of the real eframe root window, captured from `Frame` on the
    /// first update. The tray worker uses this for direct Win32 Show/Hide.
    main_hwnd: Option<usize>,
}

impl App {
    pub fn new(ctx: egui::Context) -> Self {
        let paths = Paths::resolve().expect("Failed to initialize directories");
        let settings = Settings::load(&paths.settings_file);
        let proc = ProcessHandle::new();

        let configs = paths.list_configs();
        let cores = paths.list_cores();

        let (bg_tx, bg_cmd_rx) = channel::<BgCmd>();
        let (bg_event_tx, bg_rx) = channel::<BgEvent>();
        let (log_tx, log_rx) = channel::<LogEvent>();

        // Forward LogEvents into the unified UI event channel.
        {
            let bg_event_tx = bg_event_tx.clone();
            let ctx = ctx.clone();
            thread::spawn(move || {
                while let Ok(ev) = log_rx.recv() {
                    if bg_event_tx.send(BgEvent::Log(ev)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
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
            last_error: None,
            last_info: None,
            add_dialog: AddDialogState::new(),
            delete_confirm: None,
            destroy_confirm_open: false,
            bg_tx,
            log_tx,
            bg_rx,
            tray: None,
            main_hwnd: None,
        };

        // Drop a stale selection if its folder is gone.
        if let Some(sel) = app.settings.selected_config.clone() {
            if !app.configs.iter().any(|c| c.slug == sel) {
                app.settings.selected_config = None;
                app.persist_settings();
            }
        }

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

    pub fn refresh_listings(&mut self) {
        self.configs = self.paths.list_configs();
        self.cores = self.paths.list_cores();
    }

    /// Reconcile the on-disk Windows autostart entry and the Task
    /// Scheduler "always run as administrator" task with the current
    /// `Settings`. Called from both `App::new` (startup self-heal) and
    /// `persist_settings` (after every UI mutation).
    ///
    /// Coupling rules — chosen to match the PowerToys autostart pattern:
    ///   * `always_admin` ON  + `auto_start` ON → task with LogonTrigger
    ///                                            handles the auto-launch;
    ///                                            the `HKCU\...\Run` key is
    ///                                            removed (avoids dual launch).
    ///   * `always_admin` ON  + `auto_start` OFF → on-demand task only;
    ///                                             user-launched std-user
    ///                                             instance silently
    ///                                             promotes via
    ///                                             `schtasks /run`.
    ///   * `always_admin` OFF + `auto_start` ON → no task; Run key is set.
    ///   * `always_admin` OFF + `auto_start` OFF → no task; no Run key.
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
        match self.proc.start(
            &core_exe,
            &cfg_path,
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
                    self.refresh_listings();
                    self.settings.selected_config = Some(slug.clone());
                    self.persist_settings();
                    self.last_error = None;
                    self.last_info = Some(format!("Added '{slug}'"));
                }
                BgEvent::AddDone(Err(e)) => {
                    self.add_dialog.busy = false;
                    self.last_error = Some(e);
                }
                BgEvent::EditDone(Ok(slug)) => {
                    self.add_dialog.busy = false;
                    self.add_dialog.reset();
                    self.refresh_listings();
                    self.last_error = None;
                    self.last_info = Some(format!("Updated '{slug}'"));
                }
                BgEvent::EditDone(Err(e)) => {
                    self.add_dialog.busy = false;
                    self.last_error = Some(e);
                }
                BgEvent::UpdateDone(Ok(slug)) => {
                    self.refresh_listings();
                    self.last_error = None;
                    self.last_info = Some(format!("Updated '{slug}'"));
                }
                BgEvent::UpdateDone(Err(e)) => {
                    self.last_error = Some(e);
                }
                BgEvent::DeleteDone(Ok(slug)) => {
                    if self.settings.selected_config.as_deref() == Some(&slug) {
                        self.settings.selected_config = None;
                        self.persist_settings();
                    }
                    self.refresh_listings();
                    self.last_error = None;
                    self.last_info = Some(format!("Deleted '{slug}'"));
                }
                BgEvent::DeleteDone(Err(e)) => {
                    self.last_error = Some(e);
                }
                BgEvent::SettingsSaved => {}
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Must run BEFORE any widget consumes input. Drops Enter while
        // an IME composition is active so a TextEdit doesn't surrender
        // focus mid-commit and accidentally bake the preedit (e.g. raw
        // pinyin "de'ji'd'j'e") into its buffer.
        crate::theme::swallow_enter_during_ime(ctx);

        if self.main_hwnd.is_none() {
            self.main_hwnd = main_hwnd_from_frame(frame);
            if let (Some(tray), Some(hwnd)) = (self.tray.as_ref(), self.main_hwnd) {
                tray.set_main_hwnd(hwnd);
            }
        }

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
    let mut last_auto_run = Instant::now()
        .checked_sub(Duration::from_secs(60 * 60 * 24))
        .unwrap_or_else(Instant::now);

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
                last_auto_run = Instant::now();
                ctx.request_repaint();
            }
            Ok(BgCmd::DeleteConfig(slug)) => {
                let res = run_delete(&paths, &slug, &log_tx);
                let _ = event_tx.send(BgEvent::DeleteDone(res));
                ctx.request_repaint();
            }
            Ok(BgCmd::SaveSettings(new_settings)) => {
                settings = new_settings;
                let _ = settings.save(&paths.settings_file);
                let _ = event_tx.send(BgEvent::SettingsSaved);
                ctx.request_repaint();
            }
            Ok(BgCmd::Shutdown) => break,
            Err(RecvTimeoutError::Timeout) => {
                if settings.auto_update {
                    if let Some(slug) = settings.selected_config.clone() {
                        let interval = Duration::from_secs(
                            settings.update_interval_hours.max(1).saturating_mul(3600),
                        );
                        if last_auto_run.elapsed() >= interval {
                            // Only auto-update remote entries.
                            if let Some(entry) = paths.load_entry(&slug) {
                                if matches!(entry.metadata.source, Source::Remote { .. }) {
                                    let res = updater::refresh_entry(&entry, &log_tx)
                                        .map(|_| slug.clone())
                                        .map_err(|e| e.to_string());
                                    let _ = event_tx.send(BgEvent::UpdateDone(res));
                                    ctx.request_repaint();
                                }
                            }
                            last_auto_run = Instant::now();
                        }
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
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
