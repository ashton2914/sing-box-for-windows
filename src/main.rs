// Always run windowed (no console). The launcher is a pure GUI app —
// logs go through the in-process log bus and into the UI's Logs card,
// not stdout. We disable the console for *all* builds (not just release)
// so that re-launching ourselves via ShellExecuteExW("runas") doesn't
// pop a fresh conhost window the user can't close. cargo run still
// works because the parent terminal isn't required.
#![windows_subsystem = "windows"]

mod app;
mod config;
mod core;
mod log_bus;
mod theme;
mod ui;

use eframe::egui;

/// Embedded raw RGBA bytes for the 256x256 window icon.
/// The build script rasterizes `assets/icon.svg` into `$OUT_DIR/icon.rgba`.
const ICON_RGBA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon.rgba"));
const ICON_SIDE: u32 = 256;

fn load_icon() -> egui::IconData {
    egui::IconData {
        rgba: ICON_RGBA.to_vec(),
        width: ICON_SIDE,
        height: ICON_SIDE,
    }
}

fn main() -> eframe::Result<()> {
    // Resolve once and share. `Paths::resolve` does `create_dir_all` for
    // `core/`, `config/` and `sing-box/` plus a few `current_exe` /
    // `current_dir` syscalls; the previous code called it twice in a row.
    let paths = core::paths::Paths::resolve().ok();
    let startup_settings = paths
        .as_ref()
        .map(|p| config::settings::Settings::load(&p.settings_file))
        .unwrap_or_default();

    // Persistent admin promotion. If the user previously enabled
    // "Always run as administrator", a per-user Task Scheduler task
    // with HighestAvailable run level was registered. When this
    // standard-user instance starts, hand off to that task and exit
    // before we ever create a window — the elevated copy will own the
    // session. No UAC prompt fires here.
    if paths.is_some()
        && startup_settings.always_admin
        && !core::elevation::is_elevated()
        && core::elevation::admin_task_exists()
    {
        // If the task fails for any reason (registration corrupted,
        // user removed it manually, etc.) fall through to the normal
        // standard-user launch so the user can re-enable the toggle.
        if core::elevation::run_admin_task().is_ok() {
            std::process::exit(0);
        }
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([620.0, 720.0])
            .with_min_inner_size([520.0, 560.0])
            .with_title("sing-box")
            .with_icon(load_icon())
            .with_visible(!startup_settings.silent_start),
        ..Default::default()
    };

    eframe::run_native(
        "sing-box",
        native_options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            let app = app::App::new(cc.egui_ctx.clone());
            Ok(Box::new(app))
        }),
    )
}

/// Load nicer system fonts so the UI feels closer to Material 3 typography.
/// Order: Segoe UI Variable (Win11) → Segoe UI (Win7+) → fallback to default.
/// Adds Microsoft YaHei as a CJK fallback so non-ASCII paths still render.
fn setup_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};

    let mut fonts = FontDefinitions::default();

    let primary = [
        r"C:\Windows\Fonts\SegoeUIVF.ttf",
        r"C:\Windows\Fonts\segoeui.ttf",
    ];
    for (i, path) in primary.iter().enumerate() {
        if let Ok(data) = std::fs::read(path) {
            let key = format!("seg{i}");
            fonts
                .font_data
                .insert(key.clone(), FontData::from_owned(data));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, key);
            break;
        }
    }

    let cjk = [r"C:\Windows\Fonts\msyh.ttc", r"C:\Windows\Fonts\msyh.ttf"];
    for path in cjk {
        if let Ok(data) = std::fs::read(path) {
            fonts
                .font_data
                .insert("cjk".to_owned(), FontData::from_owned(data));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .push("cjk".to_owned());
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .push("cjk".to_owned());
            break;
        }
    }

    // Monospace fallback (Cascadia Mono ships with Win10+, Consolas as backup).
    let mono = [
        r"C:\Windows\Fonts\CascadiaMono.ttf",
        r"C:\Windows\Fonts\consola.ttf",
    ];
    for (i, path) in mono.iter().enumerate() {
        if let Ok(data) = std::fs::read(path) {
            let key = format!("mono{i}");
            fonts
                .font_data
                .insert(key.clone(), FontData::from_owned(data));
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .insert(0, key);
            break;
        }
    }

    ctx.set_fonts(fonts);
}
