//! Material Design 3 (dark) theme adapter for egui.
//!
//! Provides:
//! * A baseline tonal-palette dark color scheme
//! * Typography matching the M3 type scale (Title/Body/Label sizes)
//! * Shape tokens (XS/SM/MD/LG/XL/Full)
//! * Pre-styled component helpers: `card`, `filled_button`, `tonal_button`,
//!   `outlined_button`, `text_button`, `fab`, `section_title`.
//!
//! Reference: https://m3.material.io/

#![allow(dead_code)]

use eframe::egui::{
    self, Color32, FontFamily, FontId, Margin, Rounding, Stroke, TextStyle, Vec2,
};

// -------------------------------------------------------------------------
// Color tokens (dark scheme — purple seed, M3 baseline)
// -------------------------------------------------------------------------
pub mod color {
    use eframe::egui::Color32;

    pub const PRIMARY: Color32 = Color32::from_rgb(208, 188, 255);
    pub const ON_PRIMARY: Color32 = Color32::from_rgb(56, 30, 114);
    pub const PRIMARY_CONTAINER: Color32 = Color32::from_rgb(79, 55, 139);
    pub const ON_PRIMARY_CONTAINER: Color32 = Color32::from_rgb(234, 221, 255);

    pub const SECONDARY_CONTAINER: Color32 = Color32::from_rgb(74, 68, 88);
    pub const ON_SECONDARY_CONTAINER: Color32 = Color32::from_rgb(232, 222, 248);

    pub const TERTIARY: Color32 = Color32::from_rgb(239, 184, 200);

    pub const ERROR: Color32 = Color32::from_rgb(242, 184, 181);
    pub const ON_ERROR: Color32 = Color32::from_rgb(96, 20, 16);
    pub const ERROR_CONTAINER: Color32 = Color32::from_rgb(140, 29, 24);

    pub const SUCCESS: Color32 = Color32::from_rgb(118, 217, 144);

    pub const SURFACE: Color32 = Color32::from_rgb(20, 18, 24);
    pub const SURFACE_CONTAINER_LOWEST: Color32 = Color32::from_rgb(15, 13, 19);
    pub const SURFACE_CONTAINER_LOW: Color32 = Color32::from_rgb(29, 27, 32);
    pub const SURFACE_CONTAINER: Color32 = Color32::from_rgb(33, 31, 38);
    pub const SURFACE_CONTAINER_HIGH: Color32 = Color32::from_rgb(43, 41, 48);
    pub const SURFACE_CONTAINER_HIGHEST: Color32 = Color32::from_rgb(54, 52, 59);

    pub const ON_SURFACE: Color32 = Color32::from_rgb(230, 224, 233);
    pub const ON_SURFACE_VARIANT: Color32 = Color32::from_rgb(202, 196, 208);

    pub const OUTLINE: Color32 = Color32::from_rgb(147, 143, 153);
    pub const OUTLINE_VARIANT: Color32 = Color32::from_rgb(73, 69, 79);
}

// -------------------------------------------------------------------------
// Shape tokens
// -------------------------------------------------------------------------
pub mod radius {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 28.0;
    pub const FULL: f32 = 9999.0;
}

// -------------------------------------------------------------------------
// Apply M3 style to the given egui context.
// -------------------------------------------------------------------------
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // ----- Typography (M3 type scale, slightly compacted for desktop) -----
    style.text_styles = [
        (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
    ]
    .into();

    // ----- Spacing -----
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 4.0);
    style.spacing.window_margin = Margin::same(12.0);
    style.spacing.menu_margin = Margin::same(6.0);
    style.spacing.indent = 14.0;
    style.spacing.interact_size = Vec2::new(36.0, 28.0);
    style.spacing.combo_height = 240.0;

    // ----- Visuals (dark) -----
    let mut v = egui::Visuals::dark();
    v.dark_mode = true;
    v.override_text_color = Some(color::ON_SURFACE);
    v.window_fill = color::SURFACE;
    v.panel_fill = color::SURFACE;
    v.extreme_bg_color = color::SURFACE_CONTAINER_LOWEST;
    v.faint_bg_color = color::SURFACE_CONTAINER_LOW;
    v.code_bg_color = color::SURFACE_CONTAINER;

    v.window_rounding = Rounding::same(radius::LG);
    v.menu_rounding = Rounding::same(radius::MD);
    v.window_stroke = Stroke::new(1.0, color::OUTLINE_VARIANT);
    v.window_shadow = egui::epaint::Shadow {
        offset: Vec2::new(0.0, 4.0),
        blur: 16.0,
        spread: 0.0,
        color: Color32::from_black_alpha(96),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: Vec2::new(0.0, 2.0),
        blur: 8.0,
        spread: 0.0,
        color: Color32::from_black_alpha(72),
    };

    v.selection.bg_fill = color::PRIMARY_CONTAINER;
    v.selection.stroke = Stroke::new(1.0, color::ON_PRIMARY_CONTAINER);
    v.hyperlink_color = color::PRIMARY;
    v.error_fg_color = color::ERROR;
    v.warn_fg_color = color::TERTIARY;

    // ----- Default widget look = M3 "outlined" =====
    // (filled/tonal/etc. are obtained via the helper buttons below)
    let outline = Stroke::new(1.0, color::OUTLINE);

    v.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    v.widgets.noninteractive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, color::OUTLINE_VARIANT);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, color::ON_SURFACE);
    v.widgets.noninteractive.rounding = Rounding::same(radius::SM);

    v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = outline;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, color::ON_SURFACE);
    v.widgets.inactive.rounding = Rounding::same(radius::SM);
    v.widgets.inactive.expansion = 0.0;

    v.widgets.hovered.bg_fill = with_alpha(color::ON_SURFACE, 0.08);
    v.widgets.hovered.weak_bg_fill = with_alpha(color::ON_SURFACE, 0.08);
    v.widgets.hovered.bg_stroke = outline;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, color::ON_SURFACE);
    v.widgets.hovered.rounding = Rounding::same(radius::SM);
    v.widgets.hovered.expansion = 0.0;

    v.widgets.active.bg_fill = with_alpha(color::ON_SURFACE, 0.12);
    v.widgets.active.weak_bg_fill = with_alpha(color::ON_SURFACE, 0.12);
    v.widgets.active.bg_stroke = outline;
    v.widgets.active.fg_stroke = Stroke::new(1.0, color::ON_SURFACE);
    v.widgets.active.rounding = Rounding::same(radius::SM);
    v.widgets.active.expansion = 0.0;

    v.widgets.open.bg_fill = color::SURFACE_CONTAINER_HIGH;
    v.widgets.open.weak_bg_fill = color::SURFACE_CONTAINER_HIGH;
    v.widgets.open.bg_stroke = Stroke::new(1.0, color::OUTLINE_VARIANT);
    v.widgets.open.fg_stroke = Stroke::new(1.0, color::ON_SURFACE);
    v.widgets.open.rounding = Rounding::same(radius::SM);

    style.visuals = v;
    ctx.set_style(style);
}

// -------------------------------------------------------------------------
// Helpers for painting state layers (M3's translucent overlays).
// -------------------------------------------------------------------------
fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

// -------------------------------------------------------------------------
// Component helpers
// -------------------------------------------------------------------------

/// M3 "elevated card" — Surface-Container-Low, rounded, padded.
/// Always expands to fill the available width of its parent so stacked cards
/// share the same visual column.
pub fn card<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let avail_w = ui.available_width();
    egui::Frame::none()
        .fill(color::SURFACE_CONTAINER_LOW)
        .rounding(Rounding::same(radius::LG))
        .inner_margin(Margin::same(14.0))
        .stroke(Stroke::NONE)
        .show(ui, |ui| {
            ui.set_min_width(avail_w - 28.0); // subtract horizontal inner margin (14*2)
            content(ui)
        })
        .inner
}

/// Title Large (M3) — bumped + heavy for clear card-header weight.
pub fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(color::ON_SURFACE)
            .size(20.0)
            .heading()
            .strong(),
    );
}

/// Label Medium (M3) — used for "App", "Sing-box" sub-headers.
pub fn subsection_title(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(color::ON_SURFACE_VARIANT)
            .size(13.0)
            .strong(),
    );
}

// Compact button sizing — small enough to feel refined, big enough for touch.
const BTN_MIN: Vec2 = Vec2::new(72.0, 30.0);
const BTN_MIN_TEXT: Vec2 = Vec2::new(56.0, 30.0);

/// M3 Filled Button (high emphasis, primary action).
pub fn filled_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(color::ON_PRIMARY)
            .strong(),
    )
    .fill(color::PRIMARY)
    .rounding(Rounding::same(radius::FULL))
    .min_size(BTN_MIN)
    .stroke(Stroke::new(1.0, color::PRIMARY))
}

/// M3 Filled Tonal Button (medium emphasis).
pub fn tonal_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(color::ON_SECONDARY_CONTAINER)
            .strong(),
    )
    .fill(color::SECONDARY_CONTAINER)
    .rounding(Rounding::same(radius::FULL))
    .min_size(BTN_MIN)
    .stroke(Stroke::new(1.0, color::SECONDARY_CONTAINER))
}

/// M3 Outlined Button (medium emphasis, neutral).
pub fn outlined_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(color::PRIMARY)
            .strong(),
    )
    .fill(Color32::TRANSPARENT)
    .rounding(Rounding::same(radius::FULL))
    .min_size(BTN_MIN)
    .stroke(Stroke::new(1.0, color::OUTLINE))
}

/// M3 Text Button (low emphasis).
pub fn text_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(color::PRIMARY)
            .strong(),
    )
    .fill(Color32::TRANSPARENT)
    .rounding(Rounding::same(radius::FULL))
    .min_size(BTN_MIN_TEXT)
    .stroke(Stroke::NONE)
}

/// Destructive variant of [`outlined_button`] using the error palette.
pub fn destructive_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(color::ERROR)
            .strong(),
    )
    .fill(Color32::TRANSPARENT)
    .rounding(Rounding::same(radius::FULL))
    .min_size(BTN_MIN)
    .stroke(Stroke::new(1.0, color::ERROR))
}

/// Destructive filled variant for confirm modals.
pub fn destructive_filled_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(color::ON_ERROR)
            .strong(),
    )
    .fill(color::ERROR)
    .rounding(Rounding::same(radius::FULL))
    .min_size(BTN_MIN)
    .stroke(Stroke::new(1.0, color::ERROR))
}

/// Compact circular FAB — primary container fill.
pub fn fab(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(color::ON_PRIMARY_CONTAINER)
            .size(20.0),
    )
    .fill(color::PRIMARY_CONTAINER)
    .rounding(Rounding::same(radius::FULL))
    .min_size(Vec2::new(44.0, 44.0))
    .stroke(Stroke::NONE)
}

/// 32dp icon button (square-ish, no fill).
pub fn icon_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(color::ON_SURFACE_VARIANT)
            .size(15.0),
    )
    .fill(Color32::TRANSPARENT)
    .rounding(Rounding::same(radius::FULL))
    .min_size(Vec2::new(32.0, 32.0))
    .stroke(Stroke::NONE)
}

// -------------------------------------------------------------------------
// Chip helpers — small pill labels used to badge config types in the
// dropdown. Implemented as styled runs in a `LayoutJob` so they can sit
// inside a `ComboBox` row (which only accepts a single `WidgetText`).
// -------------------------------------------------------------------------

/// Palette for a chip variant. Returned as `(background, foreground)`.
pub fn chip_palette(kind: &str) -> (Color32, Color32) {
    match kind.to_ascii_lowercase().as_str() {
        "remote" => (color::PRIMARY_CONTAINER, color::ON_PRIMARY_CONTAINER),
        _ => (color::SECONDARY_CONTAINER, color::ON_SECONDARY_CONTAINER),
    }
}

/// Append a chip-styled run to an existing `LayoutJob`.
pub fn append_chip(job: &mut egui::text::LayoutJob, kind: &str) {
    let (bg, fg) = chip_palette(kind);
    job.append(
        &format!("  {}  ", kind.to_ascii_uppercase()),
        0.0,
        egui::TextFormat {
            color: fg,
            background: bg,
            font_id: FontId::new(10.5, FontFamily::Proportional),
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
}

/// Build a `LayoutJob` that renders `name`, a small gap, then a chip badge for `kind`.
pub fn name_with_chip(name: &str, kind: &str, name_color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        name,
        0.0,
        egui::TextFormat {
            color: name_color,
            font_id: FontId::new(14.0, FontFamily::Proportional),
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
    job.append(
        "  ",
        0.0,
        egui::TextFormat {
            font_id: FontId::new(14.0, FontFamily::Proportional),
            ..Default::default()
        },
    );
    append_chip(&mut job, kind);
    job
}
