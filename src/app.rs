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
use crate::log_bus::LogEvent;

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
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
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

    pub logs: Vec<LogEvent>,
    pub last_error: Option<String>,
    pub last_info: Option<String>,
    /// Whether the log panel is currently expanded in the UI.
    pub log_visible: bool,

    pub add_dialog: AddDialogState,
    /// `Some(slug)` while the delete confirmation modal is open.
    pub delete_confirm: Option<String>,
    /// Confirm modal for destroying the working directory.
    pub destroy_confirm_open: bool,

    pub bg_tx: Sender<BgCmd>,
    pub log_tx: Sender<LogEvent>,
    bg_rx: Receiver<BgEvent>,
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
            logs: Vec::new(),
            last_error: None,
            last_info: None,
            log_visible: false,
            add_dialog: AddDialogState::new(),
            delete_confirm: None,
            destroy_confirm_open: false,
            bg_tx,
            log_tx,
            bg_rx,
        };

        // Drop a stale selection if its folder is gone.
        if let Some(sel) = app.settings.selected_config.clone() {
            if !app.configs.iter().any(|c| c.slug == sel) {
                app.settings.selected_config = None;
                app.persist_settings();
            }
        }

        if app.settings.auto_start {
            // Make sure the registry entry is present and points at the
            // current exe location — covers first launch after enabling
            // the toggle on a previous run, and the case where the user
            // moved the exe.
            let _ = crate::core::autostart::set_enabled(true);
            app.try_start();
        }
        app
    }

    pub fn refresh_listings(&mut self) {
        self.configs = self.paths.list_configs();
        self.cores = self.paths.list_cores();
    }

    pub fn persist_settings(&mut self) {
        // Mirror the "launch on Windows startup" toggle to the registry
        // alongside the on-disk save. Failures are surfaced in the UI but
        // don't block the rest of the save.
        if let Err(e) = crate::core::autostart::set_enabled(self.settings.auto_start) {
            self.last_error =
                Some(format!("Failed to update Windows autostart entry: {e}"));
        }
        let _ = self.bg_tx.send(BgCmd::SaveSettings(self.settings.clone()));
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
        let name = self.add_dialog.name.trim().to_string();
        if name.is_empty() {
            self.last_error = Some("Name is required".into());
            return;
        }
        let source = match self.add_dialog.kind {
            SourceKind::Local => {
                let path = self.add_dialog.path.trim();
                if path.is_empty() {
                    self.last_error = Some("File path is required".into());
                    return;
                }
                Source::Local {
                    path: PathBuf::from(path),
                }
            }
            SourceKind::Remote => {
                let url = self.add_dialog.url.trim();
                if url.is_empty() {
                    self.last_error = Some("URL is required".into());
                    return;
                }
                Source::Remote {
                    url: url.to_string(),
                }
            }
        };
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
        while let Ok(ev) = self.bg_rx.try_recv() {
            match ev {
                BgEvent::Log(le) => {
                    self.logs.push(le);
                    if self.logs.len() > 1000 {
                        let drop_n = self.logs.len() - 1000;
                        self.logs.drain(..drop_n);
                    }
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
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Must run BEFORE any widget consumes input. Drops Enter while
        // an IME composition is active so a TextEdit doesn't surrender
        // focus mid-commit and accidentally bake the preedit (e.g. raw
        // pinyin "de'ji'd'j'e") into its buffer.
        crate::theme::swallow_enter_during_ime(ctx);

        self.drain_events();

        egui::CentralPanel::default().show(ctx, |ui| {
            crate::ui::show(ui, self);
        });

        ctx.request_repaint_after(Duration::from_millis(750));
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
                let res = updater::add_config(&paths, &spec, &log_tx)
                    .map_err(|e| e.to_string());
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
                            if let Some(entry) = paths
                                .list_configs()
                                .into_iter()
                                .find(|e| e.slug == slug)
                            {
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

fn run_update_one(
    paths: &Paths,
    slug: &str,
    log_tx: &Sender<LogEvent>,
) -> Result<String, String> {
    let entry = paths
        .list_configs()
        .into_iter()
        .find(|e| e.slug == slug)
        .ok_or_else(|| format!("config '{slug}' not found"))?;
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
    let entry = paths
        .list_configs()
        .into_iter()
        .find(|e| e.slug == slug)
        .ok_or_else(|| format!("config '{slug}' not found"))?;
    updater::edit_entry(&entry, &spec.name, &spec.source, log_tx)
        .map(|_| slug.to_string())
        .map_err(|e| e.to_string())
}

fn run_delete(paths: &Paths, slug: &str, log_tx: &Sender<LogEvent>) -> Result<String, String> {
    let entry = paths
        .list_configs()
        .into_iter()
        .find(|e| e.slug == slug)
        .ok_or_else(|| format!("config '{slug}' not found"))?;
    updater::delete_entry(&entry, log_tx)
        .map(|_| slug.to_string())
        .map_err(|e| e.to_string())
}
