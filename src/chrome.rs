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
// Wide enough to be grabbable without precision aiming, but still narrow
// enough that normal title-bar drags don't accidentally hit a resize area.
const RESIZE_EDGE: f32 = 10.0;
const RESIZE_TOP_EDGE: f32 = 6.0;
const RESIZE_CORNER: f32 = 16.0;
// Visible window-edge frame radius. Matches the Win11 DWM corner
// radius requested in `core::win::enable_rounded_corners`, so the
// painted outline traces the actual rounded silhouette instead of
// poking out past the OS-clipped corners.
const WINDOW_CORNER_RADIUS: f32 = 8.0;
const MIN_WINDOW_WIDTH: i32 = 700;
const MIN_WINDOW_HEIGHT: i32 = 840;

#[cfg(windows)]
#[derive(Clone, Copy)]
struct WindowDragState {
    cursor_start: crate::core::win::WindowPoint,
    window_start: crate::core::win::WindowPoint,
}

#[cfg(windows)]
#[derive(Clone, Copy)]
struct WindowResizeState {
    cursor_start: crate::core::win::WindowPoint,
    window_start: crate::core::win::WindowRect,
}

/// Render the custom title bar at the top of the viewport. Must be
/// called before any other panel so it docks at the very top.
pub fn titlebar(ctx: &Context, main_hwnd: Option<usize>) {
    egui::TopBottomPanel::top("custom_titlebar")
        .exact_height(TITLEBAR_HEIGHT)
        .frame(
            egui::Frame::none()
                .fill(color::surface())
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
                let drag_id = ui.id().with("titlebar_drag");
                let drag_state_id = drag_id.with("window_drag_state");
                let drag = ui.interact(drag_rect, drag_id, Sense::click());
                if drag.double_clicked() {
                    clear_titlebar_drag_state(ui, drag_state_id);
                    let maxed = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maxed));
                } else {
                    handle_titlebar_drag(ui, ctx, drag_rect, &drag, main_hwnd, drag_state_id);
                }
            }

            // ----- App title (left) -----
            let title_font = FontId::proportional(17.0);
            let title_color = color::on_surface();
            let title_galley = ui.fonts(|f| {
                f.layout_no_wrap(
                    crate::APP_TITLE.to_string(),
                    title_font.clone(),
                    title_color,
                )
            });
            let pixels_per_point = ctx.pixels_per_point();
            let snap = |value: f32| (value * pixels_per_point).round() / pixels_per_point;
            let title_y = snap(rect.center().y - title_galley.size().y * 0.5);
            let title_left = snap(rect.left() + 18.0);
            painter.galley(egui::pos2(title_left, title_y), title_galley, title_color);

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

            painter.line_segment(
                [
                    egui::pos2(rect.left(), rect.bottom() - 0.5),
                    egui::pos2(rect.right(), rect.bottom() - 0.5),
                ],
                Stroke::new(1.0, color::outline_variant()),
            );
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
            _ => theme::blend_over(color::surface(), color::on_surface(), 0.10),
        }
    } else {
        color::surface()
    };
    if resp.hovered() {
        painter.rect_filled(rect, Rounding::ZERO, bg);
    }

    let fg = if resp.hovered() && kind == TitleButton::Close {
        Color32::WHITE
    } else {
        color::on_surface()
    };
    let stroke = Stroke::new(1.0, fg);
    let cx = rect.center().x;
    let cy = rect.center().y;
    let s = 5.0;
    let left = cx - s + 0.5;
    let right = cx + s + 0.5;
    let top = cy - s + 0.5;
    let bottom = cy + s + 0.5;

    match kind {
        TitleButton::Minimize => {
            painter.line_segment(
                [egui::pos2(left, cy + 0.5), egui::pos2(right, cy + 0.5)],
                stroke,
            );
        }
        TitleButton::Maximize => {
            paint_square_outline(&painter, left, top, right, bottom, stroke);
        }
        TitleButton::Restore => {
            // Two overlapping squares: one offset up-right, one front.
            // We mask the back square's bottom-left under the front
            // square so the result reads as the standard "restore"
            // glyph (overlapping windows).
            let off = 2.0;
            let inner_right = right - off;
            let inner_bottom = bottom - off;
            paint_square_outline(&painter, left + off, top, right, inner_bottom, stroke);
            let front = Rect::from_min_max(
                egui::pos2(left - 0.5, top + off - 0.5),
                egui::pos2(inner_right + 0.5, bottom + 0.5),
            );
            // Repaint background under the front glyph so the back
            // square's outline doesn't bleed through.
            painter.rect_filled(front, Rounding::ZERO, bg);
            paint_square_outline(&painter, left, top + off, inner_right, bottom, stroke);
        }
        TitleButton::Close => {
            // Diagonals should be symmetric around the button center.
            // The 0.5px offset used above helps horizontal/vertical
            // glyphs land on the pixel grid, but it shifts an X's
            // intersection off-center.
            let close_left = cx - s;
            let close_right = cx + s;
            let close_top = cy - s;
            let close_bottom = cy + s;
            painter.line_segment(
                [
                    egui::pos2(close_left, close_top),
                    egui::pos2(close_right, close_bottom),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(close_left, close_bottom),
                    egui::pos2(close_right, close_top),
                ],
                stroke,
            );
        }
    }

    resp
}

fn paint_square_outline(
    painter: &egui::Painter,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    stroke: Stroke,
) {
    painter.line_segment([egui::pos2(left, top), egui::pos2(right, top)], stroke);
    painter.line_segment([egui::pos2(right, top), egui::pos2(right, bottom)], stroke);
    painter.line_segment(
        [egui::pos2(right, bottom), egui::pos2(left, bottom)],
        stroke,
    );
    painter.line_segment([egui::pos2(left, bottom), egui::pos2(left, top)], stroke);
}

fn handle_titlebar_drag(
    ui: &mut egui::Ui,
    ctx: &Context,
    drag_rect: Rect,
    drag: &egui::Response,
    main_hwnd: Option<usize>,
    drag_state_id: egui::Id,
) {
    #[cfg(windows)]
    if let Some(hwnd) = main_hwnd {
        handle_titlebar_drag_windows(ui, ctx, drag_rect, hwnd, drag_state_id);
        return;
    }

    let _ = (ui, drag_rect, main_hwnd, drag_state_id);
    if drag.hovered() && ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

#[cfg(windows)]
fn handle_titlebar_drag_windows(
    ui: &mut egui::Ui,
    ctx: &Context,
    drag_rect: Rect,
    hwnd: usize,
    drag_state_id: egui::Id,
) {
    let (primary_pressed, primary_down, pointer_pos, pointer_delta) = ctx.input(|i| {
        (
            i.pointer.button_pressed(egui::PointerButton::Primary),
            i.pointer.button_down(egui::PointerButton::Primary),
            i.pointer.interact_pos().or(i.pointer.hover_pos()),
            i.pointer.delta(),
        )
    });

    if !primary_down {
        clear_titlebar_drag_state(ui, drag_state_id);
        return;
    }

    let state = ui
        .data(|data| data.get_temp::<Option<WindowDragState>>(drag_state_id))
        .flatten();

    let pointer_in_drag_region = pointer_pos.is_some_and(|pos| drag_rect.contains(pos));
    let should_start = state.is_none()
        && pointer_in_drag_region
        && (primary_pressed || pointer_delta.length_sq() > 0.0);

    let state = if should_start {
        let state = crate::core::win::cursor_pos()
            .zip(crate::core::win::window_pos(hwnd))
            .map(|(cursor_start, window_start)| WindowDragState {
                cursor_start,
                window_start,
            });
        ui.data_mut(|data| data.insert_temp(drag_state_id, state));
        state
    } else {
        state
    };

    if let Some(state) = state {
        if let Some(cursor) = crate::core::win::cursor_pos() {
            crate::core::win::set_window_pos(
                hwnd,
                state.window_start.x + cursor.x - state.cursor_start.x,
                state.window_start.y + cursor.y - state.cursor_start.y,
            );
            ctx.request_repaint();
        }
    }
}

#[cfg(windows)]
fn clear_titlebar_drag_state(ui: &mut egui::Ui, drag_state_id: egui::Id) {
    ui.data_mut(|data| data.insert_temp::<Option<WindowDragState>>(drag_state_id, None));
}

#[cfg(not(windows))]
fn clear_titlebar_drag_state(ui: &mut egui::Ui, drag_state_id: egui::Id) {
    let _ = (ui, drag_state_id);
}

/// Edge + corner resize handles. Implemented as narrow foreground
/// interaction strips instead of global pointer polling: the cursor
/// change and the drag start are now tied to the same `Response`, which
/// is what winit/eframe expects when beginning a native resize.
pub fn resize_handles(ctx: &Context, main_hwnd: Option<usize>) {
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
        main_hwnd,
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
        main_hwnd,
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
        main_hwnd,
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
        main_hwnd,
        "n",
        Rect::from_min_max(
            egui::pos2(screen.left() + c, screen.top()),
            egui::pos2(top_right_limit, screen.top() + RESIZE_TOP_EDGE),
        ),
        ResizeDirection::North,
        CursorIcon::ResizeNorth,
    );
    resize_handle(
        ctx,
        main_hwnd,
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
        main_hwnd,
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
        main_hwnd,
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
    main_hwnd: Option<usize>,
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
            handle_resize(
                ui,
                ctx,
                rect,
                &resp,
                main_hwnd,
                dir,
                egui::Id::new(("chrome_resize", id)),
            );
        });
}

fn handle_resize(
    ui: &mut egui::Ui,
    ctx: &Context,
    rect: Rect,
    resp: &egui::Response,
    main_hwnd: Option<usize>,
    dir: ResizeDirection,
    resize_id: egui::Id,
) {
    #[cfg(windows)]
    if let Some(hwnd) = main_hwnd {
        handle_resize_windows(ui, ctx, rect, hwnd, dir, resize_id);
        return;
    }

    let _ = (ui, rect, main_hwnd, resize_id);
    if resp.drag_started_by(egui::PointerButton::Primary) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
}

#[cfg(windows)]
fn handle_resize_windows(
    ui: &mut egui::Ui,
    ctx: &Context,
    rect: Rect,
    hwnd: usize,
    dir: ResizeDirection,
    resize_id: egui::Id,
) {
    let state_id = resize_id.with("window_resize_state");
    let (primary_pressed, primary_down, pointer_pos) = ctx.input(|i| {
        (
            i.pointer.button_pressed(egui::PointerButton::Primary),
            i.pointer.button_down(egui::PointerButton::Primary),
            i.pointer.interact_pos().or(i.pointer.hover_pos()),
        )
    });

    if !primary_down || !crate::core::win::left_mouse_button_down() {
        clear_resize_state(ui, state_id);
        return;
    }

    let state = ui
        .data(|data| data.get_temp::<Option<WindowResizeState>>(state_id))
        .flatten();

    let pointer_in_resize_region = pointer_pos.is_some_and(|pos| rect.contains(pos));
    let should_start = state.is_none() && primary_pressed && pointer_in_resize_region;

    let state = if should_start {
        let state = crate::core::win::cursor_pos()
            .zip(crate::core::win::window_rect(hwnd))
            .map(|(cursor_start, window_start)| WindowResizeState {
                cursor_start,
                window_start,
            });
        ui.data_mut(|data| data.insert_temp(state_id, state));
        state
    } else {
        state
    };

    if let Some(state) = state {
        if let Some(cursor) = crate::core::win::cursor_pos() {
            let dx = cursor.x - state.cursor_start.x;
            let dy = cursor.y - state.cursor_start.y;
            let mut next = state.window_start;
            match dir {
                ResizeDirection::North => next.top += dy,
                ResizeDirection::South => next.bottom += dy,
                ResizeDirection::West => next.left += dx,
                ResizeDirection::East => next.right += dx,
                ResizeDirection::NorthWest => {
                    next.top += dy;
                    next.left += dx;
                }
                ResizeDirection::NorthEast => {
                    next.top += dy;
                    next.right += dx;
                }
                ResizeDirection::SouthWest => {
                    next.bottom += dy;
                    next.left += dx;
                }
                ResizeDirection::SouthEast => {
                    next.bottom += dy;
                    next.right += dx;
                }
            }
            clamp_resize_rect(&mut next, dir);
            crate::core::win::set_window_rect(hwnd, next);
            ctx.request_repaint();
        }
    }
}

#[cfg(windows)]
fn clamp_resize_rect(rect: &mut crate::core::win::WindowRect, dir: ResizeDirection) {
    match dir {
        ResizeDirection::West | ResizeDirection::NorthWest | ResizeDirection::SouthWest => {
            rect.left = rect.left.min(rect.right - MIN_WINDOW_WIDTH);
        }
        ResizeDirection::East | ResizeDirection::NorthEast | ResizeDirection::SouthEast => {
            rect.right = rect.right.max(rect.left + MIN_WINDOW_WIDTH);
        }
        _ => {}
    }

    match dir {
        ResizeDirection::North | ResizeDirection::NorthWest | ResizeDirection::NorthEast => {
            rect.top = rect.top.min(rect.bottom - MIN_WINDOW_HEIGHT);
        }
        ResizeDirection::South | ResizeDirection::SouthWest | ResizeDirection::SouthEast => {
            rect.bottom = rect.bottom.max(rect.top + MIN_WINDOW_HEIGHT);
        }
        _ => {}
    }
}

#[cfg(windows)]
fn clear_resize_state(ui: &mut egui::Ui, state_id: egui::Id) {
    ui.data_mut(|data| data.insert_temp::<Option<WindowResizeState>>(state_id, None));
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
        Stroke::new(1.0, color::outline_variant()),
    );
}
