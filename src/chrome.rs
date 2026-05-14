//! Frameless window chrome.
//!
//! Replaces the OS-drawn title bar with a custom one that uses the same
//! M3 surface tones as the rest of the app, so the launcher reads as a
//! single unified surface from edge to edge.
//!
//! Two pieces:
//!
//! 1. [`titlebar`] — a 36px `TopBottomPanel` painted on `SURFACE`,
//!    with the app name on the left, a click-and-drag region in the
//!    middle (drag to move the window, double-click to maximise /
//!    restore), and minimise / maximise / close buttons on the right.
//!    The close button uses the Windows-conventional red hover state so
//!    it stays instantly recognisable; the other two use the M3
//!    on-surface state-layer.
//!
//! 2. [`resize_handles`] — eight invisible `Area`s along the window
//!    edges and corners that translate edge drags into
//!    `ViewportCommand::BeginResize`. Without these the window would be
//!    stuck at its launch size, because `decorated: false` removes the
//!    OS resize border too.
//!
//! Trade-off: removing the OS frame also removes Windows 11's snap-
//! layout popup that appears when you hover over the system maximise
//! button. Snap still works via Win+Arrow / drag-to-edge. Worth the
//! visual continuity.

use eframe::egui::{
    self, Align, Color32, Context, CursorIcon, FontId, Layout, Margin, Rect, ResizeDirection,
    Rounding, Sense, Stroke, Vec2, ViewportCommand,
};

use crate::theme::{self, color};

const TITLEBAR_HEIGHT: f32 = 44.0;
const BUTTON_WIDTH: f32 = 46.0;
// Wide enough to be grabbable without precision aiming. Windows 11's
// own resize border is ~8–12 px once you count the invisible "ghost"
// frame outside the visible window; with `decorated:false` we lose
// that outside slice entirely, so we have to make the inside slice
// generous instead. 10 px keeps the cursor flip predictable while
// still staying clear of the cards' content (ui.rs reserves 20 px of
// dead space on every side, and the bottom page-margin is 12 px, all
// comfortably outside this strip).
const RESIZE_EDGE: f32 = 10.0;
const RESIZE_CORNER: f32 = 22.0;
// Visible window-edge frame radius. Matches the Win11 DWM corner
// radius requested in `core::win::enable_rounded_corners`, so the
// painted outline traces the actual rounded silhouette instead of
// poking out past the OS-clipped corners.
const WINDOW_CORNER_RADIUS: f32 = 8.0;

/// Render the custom title bar at the top of the viewport. Must be
/// called before any other panel so it docks at the very top.
pub fn titlebar(ctx: &Context) {
    egui::TopBottomPanel::top("custom_titlebar")
        .exact_height(TITLEBAR_HEIGHT)
        .frame(
            egui::Frame::none()
                .fill(color::SURFACE)
                .inner_margin(Margin::ZERO),
        )
        .show_separator_line(false)
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            let painter = ui.painter().clone();
            let buttons_w = BUTTON_WIDTH * 3.0;
            let strip_left = rect.right() - buttons_w;

            // ----- Drag region (everything except the fixed window buttons) -----
            if strip_left > rect.left() {
                let drag_rect = Rect::from_min_max(
                    egui::pos2(rect.left(), rect.top()),
                    egui::pos2(strip_left, rect.bottom()),
                );
                let drag = ui.interact(
                    drag_rect,
                    ui.id().with("titlebar_drag"),
                    Sense::click_and_drag(),
                );
                if drag.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    let maxed = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maxed));
                }
            }

            // ----- App title (left) -----
            let title_font = FontId::proportional(17.0);
            let title_color = color::ON_SURFACE;
            let title_galley = ui.fonts(|f| {
                f.layout_no_wrap(
                    crate::APP_TITLE.to_string(),
                    title_font.clone(),
                    title_color,
                )
            });
            let title_y = rect.center().y - title_galley.size().y * 0.5 - 0.5;
            let title_left = rect.left() + 18.0;
            painter.galley(
                egui::pos2(title_left, title_y),
                title_galley.clone(),
                title_color,
            );
            painter.galley(
                egui::pos2(title_left + 0.45, title_y),
                title_galley,
                title_color,
            );

            // ----- Window buttons (right, fixed strip) -----
            let strip_rect = Rect::from_min_max(
                egui::pos2(strip_left, rect.top()),
                egui::pos2(rect.right(), rect.bottom()),
            );
            let mut btn_ui = ui.child_ui(strip_rect, Layout::left_to_right(Align::Center), None);
            btn_ui.spacing_mut().item_spacing = Vec2::ZERO;

            let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);

            if titlebar_button(&mut btn_ui, TitleButton::Minimize).clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            }
            let max_kind = if maximized {
                TitleButton::Restore
            } else {
                TitleButton::Maximize
            };
            if titlebar_button(&mut btn_ui, max_kind).clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
            if titlebar_button(&mut btn_ui, TitleButton::Close).clicked() {
                // Routes through eframe's normal close path so
                // `App::handle_close_request` can still intercept it
                // for the "Close button hides to tray" setting.
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TitleButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn titlebar_button(ui: &mut egui::Ui, kind: TitleButton) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(BUTTON_WIDTH, ui.available_height()),
        Sense::click(),
    );
    let painter = ui.painter().clone();

    // Compute the button's *effective* background color and paint it
    // as a solid fill. We need the value as a real color (not just a
    // semi-transparent hover layer) so the Restore glyph can use it to
    // mask the back square's outline cleanly.
    let bg = if resp.hovered() {
        match kind {
            TitleButton::Close => Color32::from_rgb(196, 43, 28),
            _ => theme::blend_over(color::SURFACE, color::ON_SURFACE, 0.10),
        }
    } else {
        color::SURFACE
    };
    if resp.hovered() {
        painter.rect_filled(rect, Rounding::ZERO, bg);
    }

    let fg = if resp.hovered() && kind == TitleButton::Close {
        Color32::WHITE
    } else {
        color::ON_SURFACE_VARIANT
    };
    let stroke = Stroke::new(1.0, fg);
    let cx = rect.center().x;
    let cy = rect.center().y;
    let s = 5.0;

    match kind {
        TitleButton::Minimize => {
            painter.line_segment(
                [egui::pos2(cx - s, cy + 0.5), egui::pos2(cx + s, cy + 0.5)],
                stroke,
            );
        }
        TitleButton::Maximize => {
            let r = Rect::from_center_size(egui::pos2(cx, cy), Vec2::splat(s * 2.0));
            painter.rect_stroke(r, Rounding::ZERO, stroke);
        }
        TitleButton::Restore => {
            // Two overlapping squares: one offset up-right, one front.
            // We mask the back square's bottom-left under the front
            // square so the result reads as the standard "restore"
            // glyph (overlapping windows).
            let off = 2.0;
            let inner = s * 2.0 - off;
            let back = Rect::from_min_size(egui::pos2(cx - s + off, cy - s), Vec2::splat(inner));
            painter.rect_stroke(back, Rounding::ZERO, stroke);
            let front = Rect::from_min_size(egui::pos2(cx - s, cy - s + off), Vec2::splat(inner));
            // Repaint background under the front glyph so the back
            // square's outline doesn't bleed through.
            painter.rect_filled(front, Rounding::ZERO, bg);
            painter.rect_stroke(front, Rounding::ZERO, stroke);
        }
        TitleButton::Close => {
            painter.line_segment(
                [egui::pos2(cx - s, cy - s), egui::pos2(cx + s, cy + s)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(cx - s, cy + s), egui::pos2(cx + s, cy - s)],
                stroke,
            );
        }
    }

    resp
}

/// Edge + corner resize handles. Implemented as narrow foreground
/// interaction strips instead of global pointer polling: the cursor
/// change and the drag start are now tied to the same `Response`, which
/// is what winit/eframe expects when beginning a native resize.
pub fn resize_handles(ctx: &Context) {
    if ctx.input(|i| i.viewport().maximized).unwrap_or(false) {
        // Maximised windows shouldn't resize from edges (and Windows
        // would refuse anyway). Skip so the cursor doesn't change to a
        // resize arrow over a non-functional area.
        return;
    }

    let screen = ctx.screen_rect();
    let e = RESIZE_EDGE;
    let c = RESIZE_CORNER;

    let buttons_left = screen.right() - BUTTON_WIDTH * 3.0;
    let top_right_limit = buttons_left.max(screen.left() + c);

    resize_handle(
        ctx,
        "nw",
        Rect::from_min_max(
            screen.left_top(),
            egui::pos2(screen.left() + c, screen.top() + c),
        ),
        ResizeDirection::NorthWest,
        CursorIcon::ResizeNwSe,
    );
    resize_handle(
        ctx,
        "sw",
        Rect::from_min_max(
            egui::pos2(screen.left(), screen.bottom() - c),
            egui::pos2(screen.left() + c, screen.bottom()),
        ),
        ResizeDirection::SouthWest,
        CursorIcon::ResizeNeSw,
    );
    resize_handle(
        ctx,
        "se",
        Rect::from_min_max(
            egui::pos2(screen.right() - c, screen.bottom() - c),
            screen.right_bottom(),
        ),
        ResizeDirection::SouthEast,
        CursorIcon::ResizeNwSe,
    );

    resize_handle(
        ctx,
        "n",
        Rect::from_min_max(
            egui::pos2(screen.left() + c, screen.top()),
            egui::pos2(top_right_limit, screen.top() + e),
        ),
        ResizeDirection::North,
        CursorIcon::ResizeNorth,
    );
    resize_handle(
        ctx,
        "s",
        Rect::from_min_max(
            egui::pos2(screen.left() + c, screen.bottom() - e),
            egui::pos2(screen.right() - c, screen.bottom()),
        ),
        ResizeDirection::South,
        CursorIcon::ResizeSouth,
    );
    resize_handle(
        ctx,
        "w",
        Rect::from_min_max(
            egui::pos2(screen.left(), screen.top() + c),
            egui::pos2(screen.left() + e, screen.bottom() - c),
        ),
        ResizeDirection::West,
        CursorIcon::ResizeWest,
    );
    resize_handle(
        ctx,
        "e",
        Rect::from_min_max(
            egui::pos2(screen.right() - e, screen.top() + TITLEBAR_HEIGHT),
            egui::pos2(screen.right(), screen.bottom() - c),
        ),
        ResizeDirection::East,
        CursorIcon::ResizeEast,
    );
}

fn resize_handle(
    ctx: &Context,
    id: &'static str,
    rect: Rect,
    dir: ResizeDirection,
    cursor: CursorIcon,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    egui::Area::new(egui::Id::new(("chrome_resize", id)))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.left_top())
        .show(ctx, |ui| {
            let (_, resp) = ui.allocate_exact_size(rect.size(), Sense::drag());
            if resp.hovered() || resp.dragged() {
                ctx.set_cursor_icon(cursor);
            }
            if resp.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
            }
        });
}

/// Paint a 1 px M3 outline around the visible window edge. Provides a
/// visual anchor for the resize zone (so users can *see* where to aim
/// instead of hunting for the invisible 10 px strip) and replaces the
/// implicit edge cue Windows normally draws as part of its native
/// chrome — which we no longer get because of `decorated: false`.
///
/// Painted on `Order::Foreground` so it sits on top of modal overlays
/// and any other Areas; we want the frame visible at all times. Skipped
/// while maximised because there is no edge there to outline (and any
/// stroke would hug the monitor bezel and look noisy).
pub fn outline(ctx: &Context) {
    if ctx.input(|i| i.viewport().maximized).unwrap_or(false) {
        return;
    }
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("chrome_window_outline"),
    ));
    // Inset by half a pixel so the 1 px stroke sits centred on the
    // pixel row at the window edge instead of straddling the boundary
    // and getting clipped to half-opacity by anti-aliasing.
    let rect = screen.shrink(0.5);
    painter.rect_stroke(
        rect,
        Rounding::same(WINDOW_CORNER_RADIUS),
        Stroke::new(1.0, color::OUTLINE_VARIANT),
    );
}
