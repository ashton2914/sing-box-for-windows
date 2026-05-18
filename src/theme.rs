//! Material Design 3 theme adapter for egui.
//!
//! Provides:
//! * Runtime-swappable tonal palettes (dark + light, M3 purple seed)
//! * Typography matching the M3 type scale (Title/Body/Label sizes)
//! * Shape tokens (XS/SM/MD/LG/Full)
//! * Pre-styled component helpers: `filled_button`, `tonal_button`,
//!   `text_button`, `fab`, `section_title`, `switch`, `segmented`.
//!
//! Reference: https://m3.material.io/

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, Color32, FontFamily, FontId, Margin, Rounding, Stroke, TextStyle, Vec2};
use serde::{Deserialize, Serialize};

// -------------------------------------------------------------------------
// Palette (runtime-swappable)
// -------------------------------------------------------------------------
//
// All M3 tonal tokens live on a `Palette` struct so we can flip dark↔light
// without recompiling. A single `AtomicBool` selects between two `static`
// palettes; reads are lock-free and the `color::*()` accessors below
// inline to a single load + field copy.
//
// Adding a token: extend `Palette`, set values in both `Palette::dark` and
// `Palette::light`, then expose a `pub fn` in `mod color`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,
    pub secondary_container: Color32,
    pub on_secondary_container: Color32,
    pub tertiary: Color32,
    pub tertiary_container: Color32,
    pub on_tertiary_container: Color32,
    pub error: Color32,
    pub on_error: Color32,
    pub error_container: Color32,
    pub success: Color32,
    pub surface: Color32,
    pub surface_container_lowest: Color32,
    pub surface_container_low: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub surface_container_highest: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
    pub is_dark: bool,
}

impl Palette {
    /// M3 baseline dark palette (purple seed).
    pub const fn dark() -> Self {
        Self {
            primary: Color32::from_rgb(208, 188, 255),
            on_primary: Color32::from_rgb(56, 30, 114),
            primary_container: Color32::from_rgb(79, 55, 139),
            on_primary_container: Color32::from_rgb(234, 221, 255),
            secondary_container: Color32::from_rgb(74, 68, 88),
            on_secondary_container: Color32::from_rgb(232, 222, 248),
            tertiary: Color32::from_rgb(239, 184, 200),
            tertiary_container: Color32::from_rgb(99, 59, 72),
            on_tertiary_container: Color32::from_rgb(255, 217, 226),
            error: Color32::from_rgb(242, 184, 181),
            on_error: Color32::from_rgb(96, 20, 16),
            error_container: Color32::from_rgb(140, 29, 24),
            success: Color32::from_rgb(118, 217, 144),
            surface: Color32::from_rgb(20, 18, 24),
            surface_container_lowest: Color32::from_rgb(15, 13, 19),
            surface_container_low: Color32::from_rgb(29, 27, 32),
            surface_container: Color32::from_rgb(33, 31, 38),
            surface_container_high: Color32::from_rgb(43, 41, 48),
            surface_container_highest: Color32::from_rgb(54, 52, 59),
            on_surface: Color32::from_rgb(230, 224, 233),
            on_surface_variant: Color32::from_rgb(202, 196, 208),
            outline: Color32::from_rgb(147, 143, 153),
            outline_variant: Color32::from_rgb(73, 69, 79),
            is_dark: true,
        }
    }

    /// M3 baseline light palette (same purple seed as `dark`).
    pub const fn light() -> Self {
        Self {
            primary: Color32::from_rgb(103, 80, 164),
            on_primary: Color32::from_rgb(255, 255, 255),
            primary_container: Color32::from_rgb(234, 221, 255),
            on_primary_container: Color32::from_rgb(33, 0, 93),
            secondary_container: Color32::from_rgb(232, 222, 248),
            on_secondary_container: Color32::from_rgb(29, 25, 43),
            tertiary: Color32::from_rgb(125, 82, 96),
            tertiary_container: Color32::from_rgb(255, 217, 226),
            on_tertiary_container: Color32::from_rgb(55, 11, 30),
            error: Color32::from_rgb(179, 38, 30),
            on_error: Color32::from_rgb(255, 255, 255),
            error_container: Color32::from_rgb(249, 222, 220),
            success: Color32::from_rgb(35, 134, 54),
            // Light-mode surface ramp follows M3 neutral-95→100 with a
            // subtle warm cast so cards visibly separate from the
            // window background.
            surface: Color32::from_rgb(254, 247, 255),
            surface_container_lowest: Color32::from_rgb(255, 255, 255),
            surface_container_low: Color32::from_rgb(247, 242, 250),
            surface_container: Color32::from_rgb(243, 237, 247),
            surface_container_high: Color32::from_rgb(236, 230, 240),
            surface_container_highest: Color32::from_rgb(230, 224, 233),
            on_surface: Color32::from_rgb(28, 27, 31),
            on_surface_variant: Color32::from_rgb(73, 69, 79),
            outline: Color32::from_rgb(121, 116, 126),
            outline_variant: Color32::from_rgb(196, 199, 197),
            is_dark: false,
        }
    }
}

static DARK_PALETTE: Palette = Palette::dark();
static LIGHT_PALETTE: Palette = Palette::light();
static IS_DARK: AtomicBool = AtomicBool::new(true);

#[inline]
pub fn current_palette() -> &'static Palette {
    if IS_DARK.load(Ordering::Relaxed) {
        &DARK_PALETTE
    } else {
        &LIGHT_PALETTE
    }
}

#[inline]
pub fn is_dark() -> bool {
    IS_DARK.load(Ordering::Relaxed)
}

#[inline]
pub fn set_dark(dark: bool) {
    IS_DARK.store(dark, Ordering::Relaxed);
}

/// User-selectable theme preference. `System` defers to the host OS.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Dark,
    Light,
    #[default]
    System,
}

impl ThemeMode {
    /// Resolve to the concrete `is_dark` flag given the current system value.
    pub fn resolve(self, system_is_dark: bool) -> bool {
        match self {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => system_is_dark,
        }
    }
}

// -------------------------------------------------------------------------
// Color tokens (M3 names → runtime palette accessors)
// -------------------------------------------------------------------------
pub mod color {
    use eframe::egui::Color32;

    use super::current_palette;

    #[inline]
    pub fn primary() -> Color32 {
        current_palette().primary
    }
    #[inline]
    pub fn on_primary() -> Color32 {
        current_palette().on_primary
    }
    #[inline]
    pub fn primary_container() -> Color32 {
        current_palette().primary_container
    }
    #[inline]
    pub fn on_primary_container() -> Color32 {
        current_palette().on_primary_container
    }
    #[inline]
    pub fn secondary_container() -> Color32 {
        current_palette().secondary_container
    }
    #[inline]
    pub fn on_secondary_container() -> Color32 {
        current_palette().on_secondary_container
    }
    #[inline]
    pub fn tertiary_container() -> Color32 {
        current_palette().tertiary_container
    }
    #[inline]
    pub fn on_tertiary_container() -> Color32 {
        current_palette().on_tertiary_container
    }
    #[inline]
    pub fn error() -> Color32 {
        current_palette().error
    }
    #[inline]
    pub fn on_error() -> Color32 {
        current_palette().on_error
    }
    #[inline]
    pub fn error_container() -> Color32 {
        current_palette().error_container
    }
    #[inline]
    pub fn success() -> Color32 {
        current_palette().success
    }
    #[inline]
    pub fn surface() -> Color32 {
        current_palette().surface
    }
    #[inline]
    pub fn surface_container_lowest() -> Color32 {
        current_palette().surface_container_lowest
    }
    #[inline]
    pub fn surface_container_low() -> Color32 {
        current_palette().surface_container_low
    }
    #[inline]
    pub fn surface_container() -> Color32 {
        current_palette().surface_container
    }
    #[inline]
    pub fn surface_container_high() -> Color32 {
        current_palette().surface_container_high
    }
    #[inline]
    pub fn surface_container_highest() -> Color32 {
        current_palette().surface_container_highest
    }
    #[inline]
    pub fn on_surface() -> Color32 {
        current_palette().on_surface
    }
    #[inline]
    pub fn on_surface_variant() -> Color32 {
        current_palette().on_surface_variant
    }
    #[inline]
    pub fn outline() -> Color32 {
        current_palette().outline
    }
    #[inline]
    pub fn outline_variant() -> Color32 {
        current_palette().outline_variant
    }
}

// -------------------------------------------------------------------------
// Shape tokens
// -------------------------------------------------------------------------
pub mod radius {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const FULL: f32 = 9999.0;
}

// -------------------------------------------------------------------------
// Apply M3 style to the given egui context.
// -------------------------------------------------------------------------
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // ----- Typography (M3 type scale, slightly compacted for desktop) -----
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(22.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        ),
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

    // ----- Visuals (palette-driven) -----
    let p = current_palette();
    let mut v = if p.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.dark_mode = p.is_dark;
    v.override_text_color = Some(p.on_surface);
    v.window_fill = p.surface;
    v.panel_fill = p.surface;
    v.extreme_bg_color = p.surface_container_low;
    v.faint_bg_color = p.surface_container_low;
    v.code_bg_color = p.surface_container;

    v.window_rounding = Rounding::same(radius::LG);
    v.menu_rounding = Rounding::same(radius::MD);
    v.window_stroke = Stroke::new(1.0, p.outline_variant);
    // Shadows are darker / more opaque on a light background so cards
    // visibly lift off the surface; on dark backgrounds we keep the
    // current subtle tint.
    let shadow_alpha = |dark: u8, light: u8| if p.is_dark { dark } else { light };
    v.window_shadow = egui::epaint::Shadow {
        offset: Vec2::new(0.0, 4.0),
        blur: 16.0,
        spread: 0.0,
        color: Color32::from_black_alpha(shadow_alpha(96, 40)),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: Vec2::new(0.0, 2.0),
        blur: 8.0,
        spread: 0.0,
        color: Color32::from_black_alpha(shadow_alpha(72, 32)),
    };

    v.selection.bg_fill = p.primary_container;
    v.selection.stroke = Stroke::new(1.0, p.on_primary_container);
    v.hyperlink_color = p.primary;
    v.error_fg_color = p.error;
    v.warn_fg_color = p.tertiary;

    // ----- Default widget look = M3 "outlined" =====
    // (filled/tonal/etc. are obtained via the helper buttons below)
    let outline = Stroke::new(1.0, p.outline);

    v.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    v.widgets.noninteractive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.outline_variant);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.on_surface);
    v.widgets.noninteractive.rounding = Rounding::same(radius::SM);

    v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = outline;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.on_surface);
    v.widgets.inactive.rounding = Rounding::same(radius::SM);
    v.widgets.inactive.expansion = 0.0;

    v.widgets.hovered.bg_fill = with_alpha(p.on_surface, 0.08);
    v.widgets.hovered.weak_bg_fill = with_alpha(p.on_surface, 0.08);
    v.widgets.hovered.bg_stroke = outline;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.on_surface);
    v.widgets.hovered.rounding = Rounding::same(radius::SM);
    v.widgets.hovered.expansion = 0.0;

    v.widgets.active.bg_fill = with_alpha(p.on_surface, 0.12);
    v.widgets.active.weak_bg_fill = with_alpha(p.on_surface, 0.12);
    v.widgets.active.bg_stroke = outline;
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.on_surface);
    v.widgets.active.rounding = Rounding::same(radius::SM);
    v.widgets.active.expansion = 0.0;

    v.widgets.open.bg_fill = p.surface_container_high;
    v.widgets.open.weak_bg_fill = p.surface_container_high;
    v.widgets.open.bg_stroke = Stroke::new(1.0, p.outline_variant);
    v.widgets.open.fg_stroke = Stroke::new(1.0, p.on_surface);
    v.widgets.open.rounding = Rounding::same(radius::SM);

    style.visuals = v;
    ctx.set_style(style);
}

// -------------------------------------------------------------------------
// Helpers for painting state layers (M3's translucent overlays).
// -------------------------------------------------------------------------
pub fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub const TRANSITION_MODAL: f32 = 0.22;
pub const TRANSITION_POPUP: f32 = 0.12;

pub fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - inv * inv * inv
}

/// Mirror of `ease_out_cubic` — fast at the start, slow at the end. Use
/// this for the *closing* half of a transition so the user immediately
/// sees the modal start to leave instead of a long static plateau.
pub fn ease_in_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| {
        ((1.0 - t) * x as f32 + t * y as f32)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ModalPhase {
    #[default]
    Closed,
    Opening,
    Open,
    Closing,
}

struct ModalTransition {
    t: f32,
    closing: bool,
    close_finished: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ModalState {
    phase: ModalPhase,
    progress: f32,
    last_tick: Option<std::time::Instant>,
}

impl ModalState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn ensure_opening(&mut self) {
        if self.phase == ModalPhase::Closed {
            self.phase = ModalPhase::Opening;
            self.progress = 0.0;
            self.last_tick = None;
        }
    }

    fn request_close(&mut self) {
        if !matches!(self.phase, ModalPhase::Closed | ModalPhase::Closing) {
            self.phase = ModalPhase::Closing;
            self.progress = self.progress.max(0.0);
            self.last_tick = None;
        }
    }

    fn is_closing(&self) -> bool {
        self.phase == ModalPhase::Closing
    }
}

fn modal_transition(ctx: &egui::Context, state: &mut ModalState) -> ModalTransition {
    state.ensure_opening();

    let now = std::time::Instant::now();
    let elapsed = state
        .last_tick
        .map(|last_tick| now.saturating_duration_since(last_tick).as_secs_f32())
        .unwrap_or(0.0)
        .min(1.0 / 20.0);
    state.last_tick = Some(now);

    let step = elapsed / TRANSITION_MODAL;
    match state.phase {
        ModalPhase::Opening => {
            state.progress = (state.progress + step).min(1.0);
            if state.progress >= 1.0 {
                state.phase = ModalPhase::Open;
            }
        }
        ModalPhase::Closing => {
            state.progress = (state.progress - step).max(0.0);
            if state.progress <= 0.0 {
                state.phase = ModalPhase::Closed;
            }
        }
        ModalPhase::Open => {
            state.progress = 1.0;
        }
        ModalPhase::Closed => {
            state.progress = 0.0;
        }
    }

    if matches!(state.phase, ModalPhase::Opening | ModalPhase::Closing) {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }

    // Asymmetric easing: ease-out while opening (fast in, soft landing),
    // ease-in while closing (immediate departure, soft tail). Sharing
    // ease_out_cubic for both halves causes a visible "plateau then jump"
    // because at progress=0.7 the multiplier is still 0.97 — the user
    // perceives no change for ~60% of the close, then a flash.
    let t = match state.phase {
        ModalPhase::Closing => ease_in_cubic(state.progress),
        _ => ease_out_cubic(state.progress),
    };

    ModalTransition {
        t,
        closing: state.is_closing(),
        close_finished: state.phase == ModalPhase::Closed,
    }
}

// -------------------------------------------------------------------------
// Component helpers
// -------------------------------------------------------------------------

/// M3 "elevated card" with an explicit outer width. Use this for page-level
/// cards whose width must be tied to the window, not to a ScrollArea's
/// transient `available_width`.
pub fn card_with_width<R>(
    ui: &mut egui::Ui,
    outer_width: f32,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let inner_w = (outer_width - 28.0).max(0.0); // subtract horizontal inner margin (14*2)
    egui::Frame::none()
        .fill(color::surface_container_low())
        .rounding(Rounding::same(radius::LG))
        .inner_margin(Margin::same(14.0))
        .stroke(Stroke::NONE)
        .show(ui, |ui| {
            ui.set_min_width(inner_w);
            ui.set_max_width(inner_w);
            content(ui)
        })
        .inner
}

/// Title Large (M3) — bumped + heavy for clear card-header weight.
pub fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(color::on_surface())
            .size(20.0)
            .heading()
            .strong(),
    );
}

/// Settings subsection header — visually below card title, above rows.
pub fn subsection_title(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(color::on_surface())
            .size(14.0)
            .strong(),
    );
}

/// Shared settings-row label style. Keep this in sync with switch labels so
/// switch rows, value rows, and picker rows read as one consistent group.
pub fn setting_label(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .color(color::on_surface())
        .size(SWITCH_LABEL_FONT)
}

// Compact button sizing for dense desktop settings/config cards.
const BTN_MIN: Vec2 = Vec2::new(64.0, 28.0);
const BTN_MIN_TEXT: Vec2 = Vec2::new(52.0, 28.0);

fn button_text(text: impl Into<String>, color: Color32) -> egui::RichText {
    egui::RichText::new(text.into())
        .color(color)
        .size(SWITCH_LABEL_FONT)
}

/// M3 Filled Button (high emphasis, primary action).
pub fn filled_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(button_text(text, color::on_primary()))
        .fill(color::primary())
        .rounding(Rounding::same(radius::FULL))
        .min_size(BTN_MIN)
        .stroke(Stroke::new(1.0, color::primary()))
}

/// M3 Filled Tonal Button (medium emphasis).
pub fn tonal_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(button_text(text, color::on_secondary_container()))
        .fill(color::secondary_container())
        .rounding(Rounding::same(radius::FULL))
        .min_size(BTN_MIN)
        .stroke(Stroke::new(1.0, color::secondary_container()))
}

/// M3 Text Button (low emphasis).
pub fn text_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(button_text(text, color::primary()))
        .fill(Color32::TRANSPARENT)
        .rounding(Rounding::same(radius::FULL))
        .min_size(BTN_MIN_TEXT)
        .stroke(Stroke::NONE)
}

/// Destructive outlined-style button using the error palette.
pub fn destructive_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(button_text(text, color::error()))
        .fill(Color32::TRANSPARENT)
        .rounding(Rounding::same(radius::FULL))
        .min_size(BTN_MIN)
        .stroke(Stroke::new(1.0, color::error()))
}

/// Destructive filled variant for confirm modals.
pub fn destructive_filled_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(button_text(text, color::on_error()))
        .fill(color::error())
        .rounding(Rounding::same(radius::FULL))
        .min_size(BTN_MIN)
        .stroke(Stroke::new(1.0, color::error()))
}

/// Compact circular FAB — primary container fill.
///
/// `kind` decides which glyph is *painted* (not laid out as text), so the
/// shape is geometrically centered inside the circle. Text glyphs like
/// `▶` have their visual mass offset from their bounding-box center, which
/// makes the play arrow look off-center; painting the triangle ourselves
/// avoids that.
#[derive(Copy, Clone)]
pub enum FabIcon {
    Play,
    Stop,
}

pub fn fab_button(ui: &mut egui::Ui, icon: FabIcon) -> egui::Response {
    let size = Vec2::new(44.0, 44.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    let visuals = ui.style().interact(&response);
    let painter = ui.painter();
    let center = rect.center();
    let radius_px = rect.width() * 0.5;

    // Background circle with hover/active state-layer overlay.
    painter.circle_filled(center, radius_px, color::primary_container());
    if response.hovered() || response.is_pointer_button_down_on() {
        painter.circle_filled(
            center,
            radius_px,
            with_alpha(color::on_primary_container(), 0.10),
        );
    }
    // Subtle focus ring.
    if response.has_focus() {
        painter.circle_stroke(center, radius_px, Stroke::new(2.0, visuals.fg_stroke.color));
    }

    let fg = color::on_primary_container();
    match icon {
        FabIcon::Play => {
            // Equilateral triangle, optically centered: shift left so the
            // visual centroid (1/3 from the base) lands on the rect center.
            let s = 16.0_f32; // base length
            let h = s * 0.866; // sqrt(3)/2 — height for equilateral
                               // Triangle points: base on the left, apex on the right.
                               // Geometric centroid is at 1/3 of the height from the base.
            let cx = center.x;
            let cy = center.y;
            let p1 = egui::pos2(cx - h / 3.0, cy - s / 2.0); // top-left
            let p2 = egui::pos2(cx - h / 3.0, cy + s / 2.0); // bottom-left
            let p3 = egui::pos2(cx + (h * 2.0) / 3.0, cy); // right apex
            painter.add(egui::Shape::convex_polygon(
                vec![p1, p2, p3],
                fg,
                Stroke::NONE,
            ));
        }
        FabIcon::Stop => {
            // Centered square, ~14px.
            let half = 6.5;
            let r = egui::Rect::from_center_size(center, Vec2::splat(half * 2.0));
            painter.rect_filled(r, Rounding::same(2.0), fg);
        }
    }

    response
}

/// A small round close button — hand-painted × via two line segments.
/// Avoids font-glyph fallback issues (e.g. U+2715 falling back to a tofu
/// box on systems whose default egui font lacks that codepoint).
pub fn close_button(ui: &mut egui::Ui) -> egui::Response {
    let size = Vec2::splat(28.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();

    // State-layer background on hover/active for subtle affordance.
    let bg = if resp.is_pointer_button_down_on() {
        with_alpha(color::on_surface(), 0.12)
    } else if resp.hovered() {
        with_alpha(color::on_surface(), 0.08)
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, Rounding::same(rect.height() * 0.5), bg);

    // Two diagonal strokes forming an ×. Inset slightly from the edges.
    let inset = 8.0;
    let stroke = Stroke::new(
        1.6,
        if resp.hovered() {
            color::on_surface()
        } else {
            color::on_surface_variant()
        },
    );
    let r = rect.shrink(inset);
    painter.line_segment([r.left_top(), r.right_bottom()], stroke);
    painter.line_segment([r.right_top(), r.left_bottom()], stroke);

    resp
}

// -------------------------------------------------------------------------
// Switch — compact desktop toggle for settings rows. Matches a 13pt label
// height; sits right-aligned in the row so the layout reads as "label …
// switch". Track 36×20, thumb 12 (off) / 16 (on). State-layer halo only
// appears while interacting and is small enough not to bleed outside the
// track edges.
// -------------------------------------------------------------------------

const SWITCH_TRACK_W: f32 = 36.0;
const SWITCH_TRACK_H: f32 = 20.0;
const SWITCH_THUMB_OFF: f32 = 12.0;
const SWITCH_THUMB_ON: f32 = 16.0;
const SWITCH_LABEL_FONT: f32 = 13.0;
const SWITCH_ROW_H: f32 = 32.0;
const SWITCH_TRANSITION: f32 = 0.24;

pub fn switch(ui: &mut egui::Ui, on: &mut bool, text: &str) -> egui::Response {
    let avail_w = ui.available_width();
    let row_size = Vec2::new(avail_w, SWITCH_ROW_H);
    let (rect, row_resp) = ui.allocate_exact_size(row_size, egui::Sense::hover());

    let painter = ui.painter();

    // Label, left-aligned and vertically centered in the row.
    let label_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            text.to_string(),
            FontId::proportional(SWITCH_LABEL_FONT),
            color::on_surface(),
        )
    });
    let label_pos = egui::pos2(rect.left(), rect.center().y - label_galley.size().y * 0.5);
    painter.galley(label_pos, label_galley, color::on_surface());

    // Track, right-aligned.
    let track_rect = egui::Rect::from_min_size(
        egui::pos2(
            rect.right() - SWITCH_TRACK_W,
            rect.center().y - SWITCH_TRACK_H * 0.5,
        ),
        Vec2::new(SWITCH_TRACK_W, SWITCH_TRACK_H),
    );
    let mut resp = ui.interact(
        track_rect.expand(4.0),
        row_resp.id.with("switch_track"),
        egui::Sense::click(),
    );
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }

    let value_t = ease_out_cubic(ui.ctx().animate_bool_with_time(
        row_resp.id.with("value_transition"),
        *on,
        SWITCH_TRANSITION,
    ));

    let track_rounding = Rounding::same(SWITCH_TRACK_H * 0.5);
    painter.rect_filled(
        track_rect,
        track_rounding,
        lerp_color(
            color::surface_container_highest(),
            color::primary(),
            value_t,
        ),
    );
    if value_t < 1.0 {
        painter.rect_stroke(
            track_rect,
            track_rounding,
            Stroke::new(
                1.5 * (1.0 - value_t),
                with_alpha(color::outline(), 1.0 - value_t),
            ),
        );
    }

    // Thumb position.
    let thumb_d = SWITCH_THUMB_OFF + (SWITCH_THUMB_ON - SWITCH_THUMB_OFF) * value_t;
    let thumb_radius = thumb_d * 0.5;
    let inset = (SWITCH_TRACK_H - thumb_d) * 0.5;
    let off_x = track_rect.left() + inset + thumb_radius;
    let on_x = track_rect.right() - inset - thumb_radius;
    let thumb_x = off_x + (on_x - off_x) * value_t;
    let thumb_center = egui::pos2(thumb_x, track_rect.center().y);

    // State layer behind the thumb — small, only when pointer is actually
    // over the switch (not just somewhere in the row).
    if let Some(alpha) = switch_layer_alpha(ui, &resp, track_rect) {
        let layer_color = lerp_color(color::on_surface(), color::primary(), value_t);
        painter.circle_filled(
            thumb_center,
            thumb_radius + 4.0,
            with_alpha(layer_color, alpha),
        );
    }

    let thumb_color = lerp_color(color::outline(), color::on_primary(), value_t);
    painter.circle_filled(thumb_center, thumb_radius, thumb_color);

    resp
}

/// Decide whether to draw the switch state-layer halo. Only the track is
/// interactive, so hovering the label cannot show a halo or toggle the value.
fn switch_layer_alpha(ui: &egui::Ui, resp: &egui::Response, track_rect: egui::Rect) -> Option<f32> {
    let pointer = ui.ctx().input(|i| i.pointer.hover_pos())?;
    let in_track = track_rect.expand(2.0).contains(pointer);
    if !in_track {
        return None;
    }
    if resp.is_pointer_button_down_on() {
        Some(0.16)
    } else if resp.hovered() {
        Some(0.08)
    } else {
        None
    }
}

// -------------------------------------------------------------------------
// Segmented picker — pill-shaped row of N options with an animated thumb.
//
// Use for small mutually-exclusive enums where a switch isn't enough but
// a dropdown would feel heavyweight (e.g. "Dark / Light / System").
// The thumb glides between cells over `SEGMENTED_TRANSITION`, intentionally
// a hair slower than `SWITCH_TRANSITION` so multi-cell travel reads
// deliberate without feeling laggy.
//
// The widget sizes itself to its content (longest label + cell padding)
// so it can sit inline next to a section label rather than spanning the
// full row width.
const SEGMENTED_HEIGHT: f32 = 24.0;
const SEGMENTED_INNER_PAD: f32 = 2.0;
const SEGMENTED_CELL_HPAD: f32 = 12.0;
const SEGMENTED_TRANSITION: f32 = 0.28;
const SEGMENTED_FONT: f32 = 11.5;

pub fn segmented<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id_source: impl std::hash::Hash,
    value: &mut T,
    options: &[(T, &str)],
) -> egui::Response {
    debug_assert!(!options.is_empty(), "segmented requires at least 1 option");
    let n = options.len().max(1) as f32;

    // Measure the widest label so every cell gets the same width and the
    // thumb glides cleanly between them.
    let font_id = FontId::proportional(SEGMENTED_FONT);
    let max_label_w = options
        .iter()
        .map(|(_, label)| {
            ui.fonts(|f| f.layout_no_wrap(label.to_string(), font_id.clone(), Color32::WHITE))
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let cell_outer_w = max_label_w + SEGMENTED_CELL_HPAD * 2.0;
    let track_w = cell_outer_w * n + SEGMENTED_INNER_PAD * 2.0;

    let (rect, mut row_resp) =
        ui.allocate_exact_size(Vec2::new(track_w, SEGMENTED_HEIGHT), egui::Sense::hover());
    let id = row_resp.id.with(id_source);

    let selected_idx = options.iter().position(|(v, _)| *v == *value).unwrap_or(0);

    // Animated thumb position (cell-index space).
    let anim = ui.ctx().animate_value_with_time(
        id.with("seg_thumb_anim"),
        selected_idx as f32,
        SEGMENTED_TRANSITION,
    );

    let track_rounding = Rounding::same(SEGMENTED_HEIGHT * 0.5);
    ui.painter()
        .rect_stroke(rect, track_rounding, Stroke::new(1.0, color::outline()));

    // Thumb pill (interpolated cell), inset from the track stroke.
    let inner = rect.shrink2(Vec2::new(SEGMENTED_INNER_PAD, SEGMENTED_INNER_PAD));
    let cell_w = inner.width() / n;
    let thumb_left = inner.left() + cell_w * anim;
    let thumb_rect = egui::Rect::from_min_size(
        egui::pos2(thumb_left, inner.top()),
        Vec2::new(cell_w, inner.height()),
    );
    let thumb_rounding = Rounding::same(inner.height() * 0.5);
    ui.painter()
        .rect_filled(thumb_rect, thumb_rounding, color::primary_container());

    // Cell click sensing + labels.
    let outer_cell_w = rect.width() / n;
    for (i, (v, label)) in options.iter().enumerate() {
        let cell = egui::Rect::from_min_size(
            egui::pos2(rect.left() + i as f32 * outer_cell_w, rect.top()),
            Vec2::new(outer_cell_w, rect.height()),
        );
        let cell_resp = ui.interact(cell, id.with(("seg_cell", i)), egui::Sense::click());
        if cell_resp.clicked() && *value != *v {
            *value = *v;
            row_resp.mark_changed();
        }
        // Hover state layer over inactive cells only.
        if cell_resp.hovered() && i != selected_idx {
            ui.painter().rect_filled(
                cell.shrink2(Vec2::new(SEGMENTED_INNER_PAD, SEGMENTED_INNER_PAD)),
                thumb_rounding,
                with_alpha(color::on_surface(), 0.06),
            );
        }
        // Label — selected cell uses on_primary_container so the text
        // stays legible against the thumb fill.
        let label_color = if i == selected_idx {
            color::on_primary_container()
        } else {
            color::on_surface()
        };
        let galley =
            ui.fonts(|f| f.layout_no_wrap(label.to_string(), font_id.clone(), label_color));
        let pos = cell.center() - galley.size() * 0.5;
        ui.painter().galley(pos, galley, label_color);
    }

    if (anim - selected_idx as f32).abs() > 0.001 {
        ui.ctx().request_repaint();
    }

    row_resp
}

// -------------------------------------------------------------------------
// Refresh / folder icon buttons — 32dp circular, hand-painted glyphs.
//
// `refresh_button` spins one full rotation on click via per-widget
// animation state stored in `ctx.data_mut`. Returns the click `Response`
// so callers can chain `.on_hover_text(…)` etc.
//
// `folder_button` paints a tiny folder pictogram — used for "Open core
// folder" so the row reads as an action affordance, not text.
// -------------------------------------------------------------------------

const CIRCULAR_ICON_SIZE: f32 = 32.0;

fn paint_state_layer(
    painter: &egui::Painter,
    resp: &egui::Response,
    rect: egui::Rect,
    radius_layer: f32,
) {
    let alpha = if resp.is_pointer_button_down_on() {
        0.12
    } else if resp.hovered() {
        0.08
    } else {
        return;
    };
    painter.circle_filled(
        rect.center(),
        radius_layer,
        with_alpha(color::on_surface(), alpha),
    );
}

pub fn refresh_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::splat(CIRCULAR_ICON_SIZE), egui::Sense::click());
    let id = resp.id;

    // Persist the click time so the spin survives across frames. Default
    // is far in the past so on first paint the icon is at rest.
    let spin_start: f64 = ui
        .ctx()
        .data(|d| d.get_temp::<f64>(id).unwrap_or(f64::NEG_INFINITY));
    if resp.clicked() {
        let now = ui.input(|i| i.time);
        ui.ctx().data_mut(|d| d.insert_temp(id, now));
    }

    let now = ui.input(|i| i.time);
    let duration = 0.6_f64;
    let elapsed = (now - spin_start).max(0.0);
    let progress = (elapsed / duration).clamp(0.0, 1.0) as f32;
    let angle = progress * std::f32::consts::TAU;
    if elapsed < duration {
        ui.ctx().request_repaint();
    }

    let painter = ui.painter().clone();
    paint_state_layer(&painter, &resp, rect, 14.0);

    // Hand-painted refresh glyph: ≈270° arc + arrowhead.
    let icon_color = color::on_surface_variant();
    let stroke = Stroke::new(1.6, icon_color);
    let center = rect.center();
    let radius_icon = 7.0;
    let rot = egui::emath::Rot2::from_angle(angle);

    let arc_start = -std::f32::consts::FRAC_PI_2; // top
    let sweep = std::f32::consts::PI * 1.5; // 270° clockwise
    let n = 24;
    let mut prev: Option<egui::Pos2> = None;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let a = arc_start + sweep * t;
        let local = Vec2::new(a.cos(), a.sin()) * radius_icon;
        let p = center + rot * local;
        if let Some(pp) = prev {
            painter.line_segment([pp, p], stroke);
        }
        prev = Some(p);
    }

    // Arrowhead at the end of the arc, tangent to the curve.
    let a_end = arc_start + sweep;
    let radial = Vec2::new(a_end.cos(), a_end.sin());
    let tangent = Vec2::new(-a_end.sin(), a_end.cos());
    let tip_local = radial * radius_icon;
    let head = 3.5;
    let p1 = tip_local - tangent * head + radial * head;
    let p2 = tip_local - tangent * head - radial * head;
    let tip = center + rot * tip_local;
    let pa = center + rot * p1;
    let pb = center + rot * p2;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, pa, pb],
        icon_color,
        Stroke::NONE,
    ));

    resp
}

pub fn folder_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::splat(CIRCULAR_ICON_SIZE), egui::Sense::click());
    let painter = ui.painter().clone();
    paint_state_layer(&painter, &resp, rect, 14.0);

    // Folder pictogram — small tab on top, body below.
    let icon_color = color::on_surface_variant();
    let stroke = Stroke::new(1.4, icon_color);
    let c = rect.center();
    let body = egui::Rect::from_center_size(egui::pos2(c.x, c.y + 1.0), Vec2::new(15.0, 11.0));
    let r = Rounding::same(1.5);
    painter.rect_stroke(body, r, stroke);
    // Tab.
    let tab = egui::Rect::from_min_size(
        egui::pos2(body.left() + 1.0, body.top() - 3.0),
        Vec2::new(6.5, 3.5),
    );
    painter.rect_filled(
        tab,
        Rounding {
            nw: 1.5,
            ne: 1.5,
            sw: 0.0,
            se: 0.0,
        },
        icon_color,
    );

    resp
}

// -------------------------------------------------------------------------
// Modal / popup dialog — single canonical look + behavior for every dialog.
//
// `modal_frame` is the shape/fill/shadow/stroke spec.
// `modal_dialog` wires up: dimmed backdrop, centered frame, custom title bar
// (left-aligned bold title + × close), Esc handling, and returns whether
// a close was requested. Use this for ALL in-app modals — never spin up
// `egui::Window` directly (it persists size between renders and gives a
// chrome we can't fully restyle).
// -------------------------------------------------------------------------

pub mod modal {
    pub const TITLE_FONT: f32 = 16.0;
    pub const HEADER_GAP: f32 = 12.0;
    pub const INNER_MARGIN: f32 = 16.0;
    pub const SHADOW_BLUR: f32 = 16.0;
    pub const BACKDROP_ALPHA: u8 = 80;
}

fn modal_frame_with_margin_opacity(inner_margin: f32, opacity: f32) -> egui::Frame {
    egui::Frame::none()
        .fill(with_alpha(color::surface_container_high(), opacity))
        .rounding(Rounding::same(radius::LG))
        .inner_margin(Margin::same(inner_margin))
        .stroke(Stroke::new(
            1.0,
            with_alpha(color::outline_variant(), opacity),
        ))
        .shadow(egui::epaint::Shadow {
            offset: Vec2::new(0.0, 4.0),
            blur: modal::SHADOW_BLUR,
            spread: 0.0,
            color: Color32::from_black_alpha((72.0 * opacity.clamp(0.0, 1.0)) as u8),
        })
}

pub struct ModalResult {
    /// True after a requested close has finished its exit animation.
    /// Caller can then dismiss the backing dialog state.
    pub close_requested: bool,
}

/// Render a modal dialog and return what its body produced plus whether its
/// close animation has completed. Backdrop blocks clicks behind the dialog.
///
/// `closable=false` disables both the × button and Esc dismissal — useful
/// for "in-flight" states (e.g. while a network add is running).
pub fn modal_dialog<R>(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    width: f32,
    closable: bool,
    add_contents: impl FnOnce(&mut egui::Ui, &mut bool) -> R,
) -> ModalResult {
    let state_id = egui::Id::new((id, "animation_state"));
    let mut state = ctx.data_mut(|data| data.get_temp::<ModalState>(state_id).unwrap_or_default());
    let result = modal_dialog_with_state(ctx, id, title, width, closable, &mut state, add_contents);
    ctx.data_mut(|data| data.insert_temp(state_id, state));
    result
}

pub fn modal_dialog_with_state<R>(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    width: f32,
    closable: bool,
    state: &mut ModalState,
    add_contents: impl FnOnce(&mut egui::Ui, &mut bool) -> R,
) -> ModalResult {
    let screen = ctx.screen_rect();
    let transition = modal_transition(ctx, state);
    let open_t = transition.t;
    let mut wants_close = false;
    egui::Area::new(egui::Id::new((id, "backdrop")))
        .order(egui::Order::Middle)
        .fixed_pos(screen.left_top())
        .fade_in(false)
        .show(ctx, |ui| {
            let resp = ui.allocate_response(screen.size(), egui::Sense::click());
            ui.painter().rect_filled(
                resp.rect,
                Rounding::ZERO,
                Color32::from_black_alpha((modal::BACKDROP_ALPHA as f32 * open_t) as u8),
            );
        });

    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .anchor(
            egui::Align2::CENTER_CENTER,
            Vec2::new(0.0, 14.0 * (1.0 - open_t)),
        )
        .fade_in(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(open_t);
            if transition.closing {
                ui.disable();
            }
            modal_frame_with_margin_opacity(modal::INNER_MARGIN, open_t)
                .show(ui, |ui| {
                    ui.set_width(width);

                    // Title bar.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(title)
                                .color(color::on_surface())
                                .size(modal::TITLE_FONT)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if closable && close_button(ui).on_hover_text("Close").clicked() {
                                wants_close = true;
                            }
                        });
                    });
                    ui.add_space(modal::HEADER_GAP);

                    add_contents(ui, &mut wants_close)
                })
                .inner
        });

    if closable && !transition.closing && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        wants_close = true;
    }
    if closable && wants_close {
        state.request_close();
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }

    ModalResult {
        close_requested: transition.close_finished,
    }
}

/// Same chrome/behavior as [`modal_dialog_with_state`], but the caller
/// controls the maximum content height. Use for long, scrollable modal
/// bodies that must track the current viewport size exactly.
#[allow(clippy::too_many_arguments)]
pub fn modal_dialog_sized_with_state<R>(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    width: f32,
    content_max_height: f32,
    frame_inner_margin: f32,
    closable: bool,
    state: &mut ModalState,
    add_contents: impl FnOnce(&mut egui::Ui, f32, &mut bool) -> R,
) -> ModalResult {
    let screen = ctx.screen_rect();
    let transition = modal_transition(ctx, state);
    let open_t = transition.t;
    let mut wants_close = false;
    egui::Area::new(egui::Id::new((id, "backdrop")))
        .order(egui::Order::Middle)
        .fixed_pos(screen.left_top())
        .fade_in(false)
        .show(ctx, |ui| {
            let resp = ui.allocate_response(screen.size(), egui::Sense::click());
            ui.painter().rect_filled(
                resp.rect,
                Rounding::ZERO,
                Color32::from_black_alpha((modal::BACKDROP_ALPHA as f32 * open_t) as u8),
            );
        });

    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .anchor(
            egui::Align2::CENTER_CENTER,
            Vec2::new(0.0, 14.0 * (1.0 - open_t)),
        )
        .fade_in(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(open_t);
            if transition.closing {
                ui.disable();
            }
            modal_frame_with_margin_opacity(frame_inner_margin, open_t)
                .show(ui, |ui| {
                    ui.set_width(width);

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(title)
                                .color(color::on_surface())
                                .size(modal::TITLE_FONT)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if closable && close_button(ui).on_hover_text("Close").clicked() {
                                wants_close = true;
                            }
                        });
                    });
                    ui.add_space(modal::HEADER_GAP);

                    add_contents(ui, content_max_height, &mut wants_close)
                })
                .inner
        });

    if closable && !transition.closing && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        wants_close = true;
    }
    if closable && wants_close {
        state.request_close();
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }

    ModalResult {
        close_requested: transition.close_finished,
    }
}

// -------------------------------------------------------------------------
// Single-line text input — IME-safe, with faded placeholder.
//
// egui's `TextEdit::singleline` treats Enter as "done" and gives up focus.
// On Windows, when an IME (Microsoft Pinyin, etc.) commits a candidate
// with Enter, that same Enter is ALSO delivered to egui as a Key::Enter
// event, which would defocus the field mid-composition. We work around
// this by re-requesting focus on the same frame whenever an `Ime` event
// (Preedit / Commit / Disabled) was observed alongside the focus loss.
// -------------------------------------------------------------------------

const INPUT_HINT_ALPHA: f32 = 0.55;
const INPUT_MARGIN: Vec2 = Vec2::new(10.0, 6.0);

/// Build a faded `RichText` suitable for use as a `TextEdit` hint — dim
/// enough to read as a placeholder, not as real input.
pub fn hint_text(s: &str) -> egui::RichText {
    egui::RichText::new(s).color(with_alpha(color::on_surface_variant(), INPUT_HINT_ALPHA))
}

/// If `response` lost focus on the same frame an IME event fired, re-grab
/// focus. Belt-and-suspenders companion to [`swallow_enter_during_ime`] —
/// useful when an IME backend skips the Preedit lifecycle and only fires
/// a Commit alongside Enter.
pub fn keep_focus_on_ime_enter(ctx: &egui::Context, response: &egui::Response) {
    if !response.lost_focus() {
        return;
    }
    let ime_event_this_frame =
        ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(_))));
    if ime_event_this_frame {
        response.request_focus();
    }
}

/// Swallow Enter key events while an IME composition is active.
///
/// Microsoft Pinyin (and most CJK IMEs) use Enter to commit a candidate.
/// In raw Win32 / browsers / native textboxes the Enter is consumed by
/// the IME and the underlying control never sees it. egui (via winit)
/// instead delivers BOTH an `Ime::Commit` event AND a `Key::Enter`
/// event, so a `TextEdit::singleline` would surrender focus mid-commit
/// — and worse, in the Preedit-then-Enter sequence the visible preedit
/// (e.g. "de'ji'd'j'e") gets baked into the buffer because focus is
/// lost before the Commit replaces it.
///
/// Call this once per frame, before any widget runs, to drop Enter
/// presses for as long as IME composition is active. Composition state
/// is tracked across frames via `ctx.data_mut`, so the Enter that
/// triggers the commit is also dropped.
pub fn swallow_enter_during_ime(ctx: &egui::Context) {
    let id = egui::Id::new("__theme_ime_composing__");
    let was_composing: bool = ctx.data(|d| d.get_temp(id).unwrap_or(false));

    let (had_ime_event, new_composing) = ctx.input(|i| {
        let mut composing = was_composing;
        let mut had = false;
        for event in &i.events {
            if let egui::Event::Ime(ime) = event {
                had = true;
                composing = match ime {
                    egui::ImeEvent::Preedit(s) => !s.is_empty(),
                    egui::ImeEvent::Commit(_) | egui::ImeEvent::Disabled => false,
                    egui::ImeEvent::Enabled => composing,
                };
            }
        }
        (had, composing)
    });

    if was_composing || had_ime_event {
        ctx.input_mut(|i| {
            i.events.retain(|e| {
                !matches!(
                    e,
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        ..
                    }
                )
            });
        });
    }

    ctx.data_mut(|d| d.insert_temp(id, new_composing));
}

/// Canonical single-line text input. Fills its column horizontally,
/// shows `hint` as a faded placeholder, and stays focused across IME
/// commits.
pub fn input_singleline(ui: &mut egui::Ui, text: &mut String, hint: &str) -> egui::Response {
    let resp = ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint_text(hint))
            .desired_width(f32::INFINITY)
            .margin(INPUT_MARGIN),
    );
    keep_focus_on_ime_enter(ui.ctx(), &resp);
    resp
}

/// Same as [`input_singleline`] but with an explicit `size` — use when the
/// input shares a row with another widget (e.g. a Browse… button).
pub fn input_singleline_sized(
    ui: &mut egui::Ui,
    text: &mut String,
    hint: &str,
    size: impl Into<Vec2>,
) -> egui::Response {
    let resp = ui.add_sized(
        size,
        egui::TextEdit::singleline(text)
            .hint_text(hint_text(hint))
            .vertical_align(egui::Align::Center)
            .margin(INPUT_MARGIN),
    );
    keep_focus_on_ime_enter(ui.ctx(), &resp);
    resp
}

pub fn setting_input_singleline_sized(
    ui: &mut egui::Ui,
    text: &mut String,
    hint: &str,
    size: impl Into<Vec2>,
) -> egui::Response {
    let resp = ui.add_sized(
        size,
        egui::TextEdit::singleline(text)
            .font(egui::FontId::proportional(SWITCH_LABEL_FONT))
            .hint_text(hint_text(hint))
            .vertical_align(egui::Align::Center)
            .margin(INPUT_MARGIN),
    );
    keep_focus_on_ime_enter(ui.ctx(), &resp);
    resp
}

// -------------------------------------------------------------------------
// Chip helpers — small rounded-pill badges painted directly on the canvas
// (not LayoutJob runs, which only produce unrounded background rectangles).
// -------------------------------------------------------------------------

const CHIP_FONT_SIZE: f32 = 10.5;
const CHIP_PAD_X: f32 = 8.0;
const CHIP_PAD_Y: f32 = 3.0;

/// Palette for a chip variant. Returned as `(background, foreground)`.
pub fn chip_palette(kind: &str) -> (Color32, Color32) {
    match kind.to_ascii_lowercase().as_str() {
        // Tertiary container — muted pink/rose. Distinct hue from primary
        // (purple) so it doesn't blend with the selected-item highlight.
        "remote" => (color::tertiary_container(), color::on_tertiary_container()),
        // Neutral surface for Local — reads as a secondary tag.
        _ => (
            color::surface_container_highest(),
            color::on_surface_variant(),
        ),
    }
}

/// The on-screen size of the chip pill for `kind` (uppercase text + padding).
pub fn chip_size(ui: &egui::Ui, kind: &str) -> Vec2 {
    let text = kind.to_ascii_uppercase();
    let font = FontId::new(CHIP_FONT_SIZE, FontFamily::Proportional);
    let galley = ui.fonts(|f| f.layout_no_wrap(text, font, Color32::WHITE));
    galley.size() + Vec2::new(CHIP_PAD_X * 2.0, CHIP_PAD_Y * 2.0)
}

/// Paint a chip pill centered on `center`. Returns the bounding rect.
pub fn paint_chip(
    ui: &egui::Ui,
    painter: &egui::Painter,
    center: egui::Pos2,
    kind: &str,
) -> egui::Rect {
    let (bg, fg) = chip_palette(kind);
    let text = kind.to_ascii_uppercase();
    let font = FontId::new(CHIP_FONT_SIZE, FontFamily::Proportional);
    let galley = ui.fonts(|f| f.layout_no_wrap(text, font, fg));
    let size = galley.size() + Vec2::new(CHIP_PAD_X * 2.0, CHIP_PAD_Y * 2.0);
    let rect = egui::Rect::from_center_size(center, size);
    painter.rect_filled(rect, Rounding::same(rect.height() * 0.5), bg);
    painter.galley(
        rect.left_top() + Vec2::new(CHIP_PAD_X, CHIP_PAD_Y),
        galley,
        fg,
    );
    rect
}
