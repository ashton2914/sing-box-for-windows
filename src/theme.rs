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

use eframe::egui::{self, Color32, FontFamily, FontId, Margin, Rounding, Stroke, TextStyle, Vec2};

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
    pub const TERTIARY_CONTAINER: Color32 = Color32::from_rgb(99, 59, 72);
    pub const ON_TERTIARY_CONTAINER: Color32 = Color32::from_rgb(255, 217, 226);

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
pub fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Composite `top` (with alpha) over solid `base`, returning the
/// resulting opaque color. Use this when you need an opaque value for
/// a state-layer blend (e.g. to paint underneath a glyph that itself
/// will mask another shape).
pub fn blend_over(base: Color32, top: Color32, top_alpha: f32) -> Color32 {
    let a = top_alpha.clamp(0.0, 1.0);
    let blend = |b: u8, t: u8| {
        ((1.0 - a) * b as f32 + a * t as f32)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(
        blend(base.r(), top.r()),
        blend(base.g(), top.g()),
        blend(base.b(), top.b()),
    )
}

pub const TRANSITION_FAST: f32 = 0.14;
pub const TRANSITION_MODAL: f32 = 0.16;
pub const TRANSITION_POPUP: f32 = 0.12;

pub fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - inv * inv * inv
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

fn modal_open_t(ctx: &egui::Context, id: egui::Id) -> f32 {
    let frame = ctx.frame_nr();
    let now = ctx.input(|i| i.time);
    let start = ctx.data_mut(|data| {
        let start = match data.get_temp::<(u64, f64)>(id) {
            Some((last_frame, start)) if last_frame + 1 >= frame => start,
            _ => now,
        };
        data.insert_temp(id, (frame, start));
        start
    });
    let predicted_dt = ctx.input(|i| i.predicted_dt);
    let t = (((now - start) as f32 + predicted_dt * 0.5) / TRANSITION_MODAL).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    ease_out_cubic(t)
}

struct ModalTransition {
    t: f32,
    closing: bool,
    close_finished: bool,
}

/// Request the standard modal exit animation for `id`. The caller should keep
/// rendering the modal until `ModalResult::close_requested` becomes true.
pub fn request_modal_close(ctx: &egui::Context, id: &str) {
    let close_id = egui::Id::new((id, "close_requested"));
    let close_start_id = egui::Id::new((id, "close_transition"));
    let frame = ctx.frame_nr();
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|data| {
        let already_closing = data.get_temp::<bool>(close_id).unwrap_or(false);
        data.insert_temp(close_id, true);
        if !already_closing {
            data.insert_temp(close_start_id, (frame, now));
        }
    });
    ctx.request_repaint();
}

fn modal_transition(ctx: &egui::Context, id: &str) -> ModalTransition {
    let frame = ctx.frame_nr();
    let now = ctx.input(|i| i.time);
    let alive_id = egui::Id::new((id, "alive"));
    let close_id = egui::Id::new((id, "close_requested"));
    let close_start_id = egui::Id::new((id, "close_transition"));

    let closing = ctx.data_mut(|data| {
        let was_alive = data
            .get_temp::<u64>(alive_id)
            .is_some_and(|last_frame| last_frame + 1 >= frame);
        data.insert_temp(alive_id, frame);
        if !was_alive {
            data.insert_temp(close_id, false);
        }
        data.get_temp::<bool>(close_id).unwrap_or(false)
    });

    if closing {
        let start = ctx.data_mut(|data| match data.get_temp::<(u64, f64)>(close_start_id) {
            Some((_, start)) => start,
            None => {
                data.insert_temp(close_start_id, (frame, now));
                now
            }
        });
        let predicted_dt = ctx.input(|i| i.predicted_dt);
        let raw = (((now - start) as f32 + predicted_dt * 0.5) / TRANSITION_MODAL).clamp(0.0, 1.0);
        if raw < 1.0 {
            ctx.request_repaint();
        }
        let close_finished = raw >= 1.0;
        if close_finished {
            ctx.data_mut(|data| {
                data.insert_temp(close_id, false);
                data.insert_temp(alive_id, 0_u64);
            });
        }
        return ModalTransition {
            t: 1.0 - ease_out_cubic(raw),
            closing: true,
            close_finished,
        };
    }

    ModalTransition {
        t: modal_open_t(ctx, egui::Id::new((id, "open_transition"))),
        closing: false,
        close_finished: false,
    }
}

// -------------------------------------------------------------------------
// Component helpers
// -------------------------------------------------------------------------

/// M3 "elevated card" — Surface-Container-Low, rounded, padded.
/// Always expands to fill the available width of its parent so stacked cards
/// share the same visual column.
pub fn card<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let avail_w = ui.available_width();
    card_with_width(ui, avail_w, content)
}

/// Same card as `card`, but with an explicit outer width. Use this for
/// page-level cards whose width must be tied to the window, not to a
/// ScrollArea's transient `available_width`.
pub fn card_with_width<R>(
    ui: &mut egui::Ui,
    outer_width: f32,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let inner_w = (outer_width - 28.0).max(0.0); // subtract horizontal inner margin (14*2)
    egui::Frame::none()
        .fill(color::SURFACE_CONTAINER_LOW)
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
            .color(color::ON_SURFACE)
            .size(20.0)
            .heading()
            .strong(),
    );
}

/// Settings subsection header — visually below card title, above rows.
pub fn subsection_title(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(color::ON_SURFACE)
            .size(14.0)
            .strong(),
    );
}

/// Shared settings-row label style. Keep this in sync with switch labels so
/// switch rows, value rows, and picker rows read as one consistent group.
pub fn setting_label(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .color(color::ON_SURFACE)
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
    egui::Button::new(button_text(text, color::ON_PRIMARY))
        .fill(color::PRIMARY)
        .rounding(Rounding::same(radius::FULL))
        .min_size(BTN_MIN)
        .stroke(Stroke::new(1.0, color::PRIMARY))
}

/// M3 Filled Tonal Button (medium emphasis).
pub fn tonal_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(button_text(text, color::ON_SECONDARY_CONTAINER))
        .fill(color::SECONDARY_CONTAINER)
        .rounding(Rounding::same(radius::FULL))
        .min_size(BTN_MIN)
        .stroke(Stroke::new(1.0, color::SECONDARY_CONTAINER))
}

/// M3 Outlined Button (medium emphasis, neutral).
pub fn outlined_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(button_text(text, color::PRIMARY))
        .fill(Color32::TRANSPARENT)
        .rounding(Rounding::same(radius::FULL))
        .min_size(BTN_MIN)
        .stroke(Stroke::new(1.0, color::OUTLINE))
}

/// M3 Text Button (low emphasis).
pub fn text_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(button_text(text, color::PRIMARY))
        .fill(Color32::TRANSPARENT)
        .rounding(Rounding::same(radius::FULL))
        .min_size(BTN_MIN_TEXT)
        .stroke(Stroke::NONE)
}

/// Destructive variant of [`outlined_button`] using the error palette.
pub fn destructive_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(button_text(text, color::ERROR))
        .fill(Color32::TRANSPARENT)
        .rounding(Rounding::same(radius::FULL))
        .min_size(BTN_MIN)
        .stroke(Stroke::new(1.0, color::ERROR))
}

/// Destructive filled variant for confirm modals.
pub fn destructive_filled_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(button_text(text, color::ON_ERROR))
        .fill(color::ERROR)
        .rounding(Rounding::same(radius::FULL))
        .min_size(BTN_MIN)
        .stroke(Stroke::new(1.0, color::ERROR))
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
    painter.circle_filled(center, radius_px, color::PRIMARY_CONTAINER);
    if response.hovered() || response.is_pointer_button_down_on() {
        painter.circle_filled(
            center,
            radius_px,
            with_alpha(color::ON_PRIMARY_CONTAINER, 0.10),
        );
    }
    // Subtle focus ring.
    if response.has_focus() {
        painter.circle_stroke(center, radius_px, Stroke::new(2.0, visuals.fg_stroke.color));
    }

    let fg = color::ON_PRIMARY_CONTAINER;
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

/// A small round close button — hand-painted × via two line segments.
/// Avoids font-glyph fallback issues (e.g. U+2715 falling back to a tofu
/// box on systems whose default egui font lacks that codepoint).
pub fn close_button(ui: &mut egui::Ui) -> egui::Response {
    let size = Vec2::splat(28.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();

    // State-layer background on hover/active for subtle affordance.
    let bg = if resp.is_pointer_button_down_on() {
        with_alpha(color::ON_SURFACE, 0.12)
    } else if resp.hovered() {
        with_alpha(color::ON_SURFACE, 0.08)
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, Rounding::same(rect.height() * 0.5), bg);

    // Two diagonal strokes forming an ×. Inset slightly from the edges.
    let inset = 8.0;
    let stroke = Stroke::new(
        1.6,
        if resp.hovered() {
            color::ON_SURFACE
        } else {
            color::ON_SURFACE_VARIANT
        },
    );
    let r = rect.shrink(inset);
    painter.line_segment([r.left_top(), r.right_bottom()], stroke);
    painter.line_segment([r.right_top(), r.left_bottom()], stroke);

    resp
}

// -------------------------------------------------------------------------
// Checkbox — single canonical M3-style checkbox used everywhere.
//
// Hand-painted to bypass egui's default toggle look (a tall stroked square
// with no fill / no state-layer). Geometry:
//   * 18dp rounded square box
//   * 2dp stroke when unchecked (ON_SURFACE_VARIANT)
//   * Solid PRIMARY fill + ON_PRIMARY checkmark when checked
//   * Round state-layer (8% / 16% ON_SURFACE) on hover/active
//   * 8dp gap, then a 13pt label in ON_SURFACE
// -------------------------------------------------------------------------

const CHECKBOX_BOX: f32 = 18.0;
const CHECKBOX_GAP: f32 = 8.0;
const CHECKBOX_LABEL_FONT: f32 = 13.0;

pub fn checkbox(ui: &mut egui::Ui, checked: &mut bool, text: &str) -> egui::Response {
    let label_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            text.to_string(),
            FontId::proportional(CHECKBOX_LABEL_FONT),
            color::ON_SURFACE,
        )
    });
    let total = Vec2::new(
        CHECKBOX_BOX + CHECKBOX_GAP + label_galley.size().x,
        label_galley.size().y.max(CHECKBOX_BOX),
    );
    let (rect, mut resp) = ui.allocate_exact_size(total, egui::Sense::click());

    if resp.clicked() {
        *checked = !*checked;
        resp.mark_changed();
    }

    let painter = ui.painter();
    let box_center = egui::pos2(rect.left() + CHECKBOX_BOX * 0.5, rect.center().y);
    let box_rect = egui::Rect::from_center_size(box_center, Vec2::splat(CHECKBOX_BOX));

    // Round state-layer behind the box.
    if resp.is_pointer_button_down_on() {
        painter.circle_filled(
            box_center,
            CHECKBOX_BOX * 0.85,
            with_alpha(color::ON_SURFACE, 0.16),
        );
    } else if resp.hovered() {
        painter.circle_filled(
            box_center,
            CHECKBOX_BOX * 0.85,
            with_alpha(color::ON_SURFACE, 0.08),
        );
    }

    let r = Rounding::same(3.0);
    if *checked {
        painter.rect_filled(box_rect, r, color::PRIMARY);
        // Two-segment checkmark.
        let stroke = Stroke::new(2.0, color::ON_PRIMARY);
        let tl = box_rect.left_top();
        let p1 = tl + Vec2::new(CHECKBOX_BOX * 0.22, CHECKBOX_BOX * 0.52);
        let p2 = tl + Vec2::new(CHECKBOX_BOX * 0.42, CHECKBOX_BOX * 0.72);
        let p3 = tl + Vec2::new(CHECKBOX_BOX * 0.78, CHECKBOX_BOX * 0.32);
        painter.line_segment([p1, p2], stroke);
        painter.line_segment([p2, p3], stroke);
    } else {
        let stroke = Stroke::new(2.0, color::ON_SURFACE_VARIANT);
        painter.rect_stroke(box_rect, r, stroke);
    }

    let text_pos = egui::pos2(
        box_rect.right() + CHECKBOX_GAP,
        rect.center().y - label_galley.size().y * 0.5,
    );
    painter.galley(text_pos, label_galley, color::ON_SURFACE);

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
const SWITCH_ROW_H: f32 = 28.0;
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
            color::ON_SURFACE,
        )
    });
    let label_pos = egui::pos2(rect.left(), rect.center().y - label_galley.size().y * 0.5);
    painter.galley(label_pos, label_galley, color::ON_SURFACE);

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
        lerp_color(color::SURFACE_CONTAINER_HIGHEST, color::PRIMARY, value_t),
    );
    if value_t < 1.0 {
        painter.rect_stroke(
            track_rect,
            track_rounding,
            Stroke::new(
                1.5 * (1.0 - value_t),
                with_alpha(color::OUTLINE, 1.0 - value_t),
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
        let layer_color = lerp_color(color::ON_SURFACE, color::PRIMARY, value_t);
        painter.circle_filled(
            thumb_center,
            thumb_radius + 4.0,
            with_alpha(layer_color, alpha),
        );
    }

    let thumb_color = lerp_color(color::OUTLINE, color::ON_PRIMARY, value_t);
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
        with_alpha(color::ON_SURFACE, alpha),
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
    let icon_color = color::ON_SURFACE_VARIANT;
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
    let icon_color = color::ON_SURFACE_VARIANT;
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
    pub const SHADOW_BLUR: f32 = 24.0;
    pub const BACKDROP_ALPHA: u8 = 80;
}

/// The standard modal frame: SURFACE_CONTAINER_HIGH fill, LG rounding,
/// OUTLINE_VARIANT 1dp stroke, deep soft shadow.
pub fn modal_frame() -> egui::Frame {
    modal_frame_with_margin(modal::INNER_MARGIN)
}

/// Modal frame with caller-controlled inner margin. Kept separate from
/// [`modal_frame`] so small viewport dialogs can reduce padding without
/// changing the canonical look of normal confirmation/input dialogs.
pub fn modal_frame_with_margin(inner_margin: f32) -> egui::Frame {
    egui::Frame::none()
        .fill(color::SURFACE_CONTAINER_HIGH)
        .rounding(Rounding::same(radius::LG))
        .inner_margin(Margin::same(inner_margin))
        .stroke(Stroke::new(1.0, color::OUTLINE_VARIANT))
        .shadow(egui::epaint::Shadow {
            offset: Vec2::new(0.0, 8.0),
            blur: modal::SHADOW_BLUR,
            spread: 0.0,
            color: Color32::from_black_alpha(120),
        })
}

pub struct ModalResult<R> {
    /// True after a requested close has finished its exit animation.
    /// Caller can then dismiss the backing dialog state.
    pub close_requested: bool,
    pub inner: R,
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
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> ModalResult<R> {
    let screen = ctx.screen_rect();
    let transition = modal_transition(ctx, id);
    let open_t = transition.t;
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

    let area = egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .anchor(
            egui::Align2::CENTER_CENTER,
            Vec2::new(0.0, -8.0 * (1.0 - open_t)),
        )
        .fade_in(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(open_t);
            if transition.closing {
                ui.disable();
            }
            modal_frame()
                .show(ui, |ui| {
                    ui.set_width(width);

                    // Title bar.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(title)
                                .color(color::ON_SURFACE)
                                .size(modal::TITLE_FONT)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if closable && close_button(ui).on_hover_text("Close").clicked() {
                                request_modal_close(ctx, id);
                            }
                        });
                    });
                    ui.add_space(modal::HEADER_GAP);

                    add_contents(ui)
                })
                .inner
        });

    if closable && !transition.closing && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        request_modal_close(ctx, id);
    }

    ModalResult {
        close_requested: transition.close_finished,
        inner: area.inner,
    }
}

/// Same chrome/behavior as [`modal_dialog`], but the caller controls the
/// maximum content height. Use for long, scrollable modal bodies that must
/// track the current viewport size exactly.
pub fn modal_dialog_sized<R>(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    width: f32,
    content_max_height: f32,
    frame_inner_margin: f32,
    closable: bool,
    add_contents: impl FnOnce(&mut egui::Ui, f32) -> R,
) -> ModalResult<R> {
    let screen = ctx.screen_rect();
    let transition = modal_transition(ctx, id);
    let open_t = transition.t;
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

    let area = egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .anchor(
            egui::Align2::CENTER_CENTER,
            Vec2::new(0.0, -8.0 * (1.0 - open_t)),
        )
        .fade_in(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(open_t);
            if transition.closing {
                ui.disable();
            }
            modal_frame_with_margin(frame_inner_margin)
                .show(ui, |ui| {
                    ui.set_width(width);

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(title)
                                .color(color::ON_SURFACE)
                                .size(modal::TITLE_FONT)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if closable && close_button(ui).on_hover_text("Close").clicked() {
                                request_modal_close(ctx, id);
                            }
                        });
                    });
                    ui.add_space(modal::HEADER_GAP);

                    add_contents(ui, content_max_height)
                })
                .inner
        });

    if closable && !transition.closing && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        request_modal_close(ctx, id);
    }

    ModalResult {
        close_requested: transition.close_finished,
        inner: area.inner,
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
    egui::RichText::new(s).color(with_alpha(color::ON_SURFACE_VARIANT, INPUT_HINT_ALPHA))
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
        "remote" => (color::TERTIARY_CONTAINER, color::ON_TERTIARY_CONTAINER),
        // Neutral surface for Local — reads as a secondary tag.
        _ => (color::SURFACE_CONTAINER_HIGHEST, color::ON_SURFACE_VARIANT),
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
