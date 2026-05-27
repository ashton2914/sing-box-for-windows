use eframe::egui;

use crate::app::{App, BgCmd, SourceKind};
use crate::config::entry::Source;
use crate::config::settings::{
    InboundOverrideKind, LogLevel, DEFAULT_MIXED_LISTEN, DEFAULT_MIXED_LISTEN_PORT,
    DEFAULT_UPDATE_INTERVAL_HOURS,
};
use crate::core::shell;
use crate::log_bus::LogEvent;
use crate::theme::{self, color, radius};

#[derive(Clone, Copy)]
struct MainScrollState {
    viewport_height: f32,
    content_height: f32,
    offset_y: f32,
}

impl Default for MainScrollState {
    fn default() -> Self {
        Self {
            viewport_height: 0.0,
            content_height: 0.0,
            offset_y: 0.0,
        }
    }
}

const MAIN_SCROLL_BOTTOM_MARGIN: f32 = 12.0;
const SETTINGS_INPUT_HEIGHT: f32 = 28.0;
const SETTINGS_INPUT_SHORT: [f32; 2] = [72.0, SETTINGS_INPUT_HEIGHT];
const SETTINGS_INPUT_MEDIUM: [f32; 2] = [104.0, SETTINGS_INPUT_HEIGHT];
const SETTINGS_INPUT_ADDRESS: [f32; 2] = [180.0, SETTINGS_INPUT_HEIGHT];
const SETTINGS_ROW_HEIGHT: f32 = 28.0;
const SETTINGS_ACTION_ROW_HEIGHT: f32 = 28.0;
const SETTINGS_CHILD_INDENT: f32 = 18.0;

fn log_scroll_blocking_rect_id() -> egui::Id {
    egui::Id::new("log_scroll_blocking_rect")
}

fn popup_scroll_blocking_id() -> egui::Id {
    egui::Id::new("popup_scroll_blocking")
}

fn modal_window_open(app: &App) -> bool {
    app.add_dialog.open
        || app.delete_confirm.is_some()
        || app.destroy_confirm_open
        || app.about_open
        || app.core_version_open
}

fn settings_row(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    settings_row_sized(ui, SETTINGS_ROW_HEIGHT, add_contents);
}

fn settings_row_sized(ui: &mut egui::Ui, height: f32, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), height),
        egui::Layout::left_to_right(egui::Align::Center),
        add_contents,
    );
}

fn settings_child_row(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    settings_row(ui, |ui| {
        ui.add_space(SETTINGS_CHILD_INDENT);
        add_contents(ui);
    });
}

pub fn show(ui: &mut egui::Ui, app: &mut App) {
    // Keep the stacked page cards visually centered in the actual
    // CentralPanel viewport. This deliberately uses the parent UI width
    // instead of `ctx.screen_rect().width()`: the screen rect is a window
    // coordinate, while ScrollArea/content layout is panel-local. Mixing
    // those coordinate systems is what made the right inset look tighter
    // after the main scrollbar was hidden.
    const PAGE_INSET: f32 = 20.0;

    egui::Frame::none()
        .inner_margin(egui::Margin {
            left: 0.0,
            right: 0.0,
            top: 6.0,
            bottom: 0.0,
        })
        .show(ui, |ui| {
            ui.spacing_mut().scroll.floating = true;
            let page_width = ui.available_width();
            let card_width = (page_width - PAGE_INSET * 2.0).max(240.0);
            let scroll_id = ui.make_persistent_id("main_scroll");
            let viewport_size = egui::vec2(page_width, ui.available_height());
            let (viewport_rect, _) = ui.allocate_exact_size(viewport_size, egui::Sense::hover());

            let mut scroll_state = ui
                .data(|data| data.get_temp::<MainScrollState>(scroll_id))
                .unwrap_or_default();
            let max_offset = (scroll_state.content_height - viewport_rect.height()).max(0.0);
            let popup_scroll_blocked = ui.ctx().data(|data| {
                data.get_temp::<bool>(popup_scroll_blocking_id())
                    .unwrap_or(false)
            });
            ui.ctx()
                .data_mut(|data| data.insert_temp(popup_scroll_blocking_id(), false));
            let background_scroll_blocked = modal_window_open(app) || popup_scroll_blocked;
            // Use egui's `smooth_scroll_delta`: it folds notched wheel ticks
            // into a short easing tail so the page doesn't stair-step. The
            // earlier raw-delta variant felt jittery, and a custom
            // exponential approach felt unnatural, so we trust egui's tail.
            let scroll_delta_y = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
            let pointer_in_viewport = pointer_pos.is_some_and(|pos| viewport_rect.contains(pos));
            let pointer_over_log_scroll = pointer_pos.is_some_and(|pos| {
                ui.ctx().data(|data| {
                    data.get_temp::<egui::Rect>(log_scroll_blocking_rect_id())
                        .is_some_and(|rect| rect.contains(pos))
                })
            });

            if pointer_in_viewport
                && !background_scroll_blocked
                && !pointer_over_log_scroll
                && scroll_delta_y.abs() > 0.0
            {
                let requested = scroll_state.offset_y - scroll_delta_y;
                scroll_state.offset_y = if requested > scroll_state.offset_y {
                    requested.min(max_offset.max(scroll_state.offset_y))
                } else {
                    requested.max(0.0)
                };
            }

            let content_layout_height = if scroll_state.content_height > 0.0 {
                scroll_state
                    .content_height
                    .max(scroll_state.offset_y + viewport_rect.height())
            } else {
                viewport_rect.height()
            };
            let content_rect = egui::Rect::from_min_size(
                viewport_rect.left_top() - egui::vec2(0.0, scroll_state.offset_y),
                egui::vec2(viewport_rect.width(), content_layout_height),
            );
            let mut content_ui = ui.child_ui(content_rect, *ui.layout(), None);
            let content_top = content_rect.top();

            let content = egui::Frame::none()
                .inner_margin(egui::Margin {
                    left: PAGE_INSET,
                    right: PAGE_INSET,
                    top: 0.0,
                    bottom: MAIN_SCROLL_BOTTOM_MARGIN,
                })
                .show(&mut content_ui, |ui| {
                    run_card(ui, app, card_width);
                    ui.add_space(8.0);
                    config_card(ui, app, card_width);
                    ui.add_space(8.0);
                    settings_card(ui, app, card_width);
                    ui.min_rect().bottom() - content_top + MAIN_SCROLL_BOTTOM_MARGIN
                });

            scroll_state.viewport_height = viewport_rect.height();
            scroll_state.content_height = content.inner.max(0.0);
            if scroll_state.offset_y <= 0.5 {
                scroll_state.offset_y = 0.0;
            }
            ui.data_mut(|data| data.insert_temp(scroll_id, scroll_state));
        });

    add_config_modal(ui.ctx(), app);
    delete_confirm_modal(ui.ctx(), app);
    destroy_confirm_modal(ui.ctx(), app);
    about_modal(ui.ctx(), app);
    core_version_modal(ui.ctx(), app);
}

// ---------- Config card ----------

fn config_card(ui: &mut egui::Ui, app: &mut App, card_width: f32) {
    theme::card_with_width(ui, card_width, |ui| {
        theme::section_title(ui, "Configs");
        ui.add_space(6.0);

        // ----- selector + Add button on one row -----
        // The Add button is placed first in a right-to-left layout so it
        // anchors to the right edge; `config_combo` then fills the
        // remaining space to its left.
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 36.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                if ui.add(theme::filled_button("+ Add")).clicked() {
                    app.add_dialog.reset();
                    app.add_dialog.open = true;
                }
                let remaining = ui.available_width();
                config_combo(ui, app, remaining);
            },
        );

        // ----- detail panel -----
        ui.add_space(8.0);
        match app.selected_entry().cloned() {
            None => {
                ui.label(
                    egui::RichText::new("Select a config above, or click + Add to create one.")
                        .color(color::on_surface_variant()),
                );
            }
            Some(entry) => {
                let is_remote = matches!(entry.metadata.source, Source::Remote { .. });
                let last = entry
                    .metadata
                    .last_updated
                    .clone()
                    .unwrap_or_else(|| "(never)".to_string());

                egui::Grid::new("config_detail_grid")
                    .num_columns(2)
                    .spacing([14.0, 4.0])
                    .show(ui, |ui| {
                        detail_value(ui, "Last updated");
                        detail_value(ui, &last);
                        ui.end_row();
                    });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    // Primary action depends on the source kind:
                    //   * Remote → "Update" (re-download from the URL).
                    //   * Local  → "Replace" (pick a new file on disk).
                    // Both keep the same slug + display name so the user
                    // selection in the combo box stays valid.
                    let primary_label = if is_remote { "Update" } else { "Replace" };
                    if ui.add(theme::filled_button(primary_label)).clicked() {
                        if is_remote {
                            let _ = app.bg_tx.send(BgCmd::UpdateConfig(entry.slug.clone()));
                        } else if let Some(p) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            // Reuse the EditConfig path: changing the
                            // recorded source path causes updater to
                            // re-fetch the file and bump last_updated,
                            // exactly what "replace" needs.
                            let spec = crate::config::updater::NewConfigSpec {
                                name: entry.metadata.name.clone(),
                                source: Source::Local { path: p },
                            };
                            let _ = app.bg_tx.send(BgCmd::EditConfig {
                                slug: entry.slug.clone(),
                                spec,
                            });
                        }
                    }
                    if ui.add(theme::tonal_button("Edit")).clicked() {
                        app.add_dialog.open_for_edit(&entry);
                    }
                    if ui.add(theme::tonal_button("Save As")).clicked() {
                        // Default file name = the user's display name with
                        // a `.json` suffix. The native save dialog will
                        // surface any illegal characters to the user
                        // before we hit `fs::copy`.
                        let default_name = format!("{}.json", entry.metadata.name);
                        if let Some(dest) = rfd::FileDialog::new()
                            .set_file_name(&default_name)
                            .add_filter("JSON", &["json"])
                            .save_file()
                        {
                            match std::fs::copy(entry.config_file(), &dest) {
                                Ok(_) => {
                                    app.last_error = None;
                                    app.last_info = Some(format!("Saved to {}", dest.display()));
                                }
                                Err(e) => {
                                    app.last_error = Some(format!("Save As failed: {e}"));
                                }
                            }
                        }
                    }
                    if ui.add(theme::destructive_button("Delete")).clicked() {
                        app.delete_confirm = Some(entry.slug.clone());
                    }
                });
            }
        }
    });
}

/// Hand-rolled combo widget shared by `config_combo` and `core_combo`.
///
/// egui's built-in `ComboBox` only accepts a single `WidgetText` for the
/// closed state and forces a popup chrome we can't theme. Both of our
/// selectors want a custom paint (chip after the name, etc.) and a popup
/// that visually glues to the closed combo, so we paint the whole shell
/// here once instead of duplicating ~150 lines per selector.
///
/// The shell handles:
///   * outlined surface tile with hover state-layer when closed,
///   * adaptive corner rounding (bottom corners flatten while open so the
///     popup frame can dock on with matching top-flat corners),
///   * a centered chevron on the right,
///   * popup `Area` placement, dismissal on outside click, and the
///     `ui.memory` open/close bookkeeping.
///
/// `paint_closed` paints the left-of-chevron region of the closed combo
/// (called every frame). `render_items` paints the popup body (called
/// only while open) and returns `Some(value)` when the user clicked a
/// selectable row, which becomes this function's return value.
fn popup_combo<T>(
    ui: &mut egui::Ui,
    id_source: &'static str,
    width: f32,
    height: f32,
    paint_closed: impl FnOnce(&mut egui::Ui, &egui::Painter, egui::Rect, /*content_right*/ f32),
    render_items: impl FnOnce(&mut egui::Ui) -> Option<T>,
) -> Option<T> {
    let popup_id = ui.make_persistent_id(id_source);
    let mut is_open = ui.memory(|m| m.is_popup_open(popup_id));

    let response = ui.allocate_response(egui::vec2(width, height), egui::Sense::click());
    let rect = response.rect;
    let painter = ui.painter().clone();

    if response.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
        is_open = ui.memory(|m| m.is_popup_open(popup_id));
    }

    let open_t = theme::ease_out_cubic(ui.ctx().animate_bool_with_time(
        popup_id.with("open_transition"),
        is_open,
        theme::TRANSITION_POPUP,
    ));
    if is_open || open_t > 0.01 {
        ui.ctx()
            .data_mut(|data| data.insert_temp(popup_scroll_blocking_id(), true));
    }

    let outline = egui::Stroke::new(1.0, color::outline_variant());
    let surface = color::surface_container();

    // Closed-combo rounding: bottom corners ease flat as the popup opens.
    let bottom_radius = radius::SM * (1.0 - open_t);
    let combo_rounding = egui::Rounding {
        nw: radius::SM,
        ne: radius::SM,
        sw: bottom_radius,
        se: bottom_radius,
    };
    painter.rect(rect, combo_rounding, surface, outline);
    if response.hovered() && !is_open {
        painter.rect_filled(
            rect,
            combo_rounding,
            theme::with_alpha(color::on_surface(), 0.04),
        );
    }

    let inner = rect.shrink2(egui::vec2(12.0, 0.0));
    let center_y = inner.center().y;

    // Chevron (down-triangle) on the right.
    let chev_color = color::on_surface_variant();
    let chev_w = 9.0;
    let chev_h = 5.0;
    let chev_cx = inner.right() - chev_w * 0.5;
    let chev_center = egui::pos2(chev_cx, center_y);
    let angle = std::f32::consts::PI * open_t;
    let (sin, cos) = angle.sin_cos();
    let rotate = |dx: f32, dy: f32| {
        egui::pos2(
            chev_center.x + dx * cos - dy * sin,
            chev_center.y + dx * sin + dy * cos,
        )
    };
    let chev = vec![
        rotate(-chev_w * 0.5, -chev_h * 0.5),
        rotate(chev_w * 0.5, -chev_h * 0.5),
        rotate(0.0, chev_h * 0.5),
    ];
    painter.add(egui::Shape::convex_polygon(
        chev,
        chev_color,
        egui::Stroke::NONE,
    ));
    let content_right = inner.right() - chev_w - 8.0;

    paint_closed(ui, &painter, inner, content_right);

    let mut picked: Option<T> = None;
    let mut close_requested = false;

    if is_open || open_t > 0.01 {
        let popup_offset = egui::vec2(0.0, -6.0 * (1.0 - open_t));
        let area = egui::Area::new(popup_id.with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.left_bottom() + popup_offset)
            .fade_in(false)
            .show(ui.ctx(), |ui| {
                ui.multiply_opacity(open_t);
                egui::Frame::none()
                    .fill(surface)
                    .stroke(outline)
                    .rounding(egui::Rounding {
                        nw: 0.0,
                        ne: 0.0,
                        sw: radius::SM,
                        se: radius::SM,
                    })
                    .inner_margin(egui::Margin::symmetric(4.0, 6.0))
                    .show(ui, |ui| {
                        ui.set_min_width(width - 8.0);
                        ui.set_max_width(width - 8.0);
                        if let Some(p) = render_items(ui) {
                            picked = Some(p);
                            close_requested = true;
                        }
                    });
            });

        let outside_click = ui.input(|i| {
            i.pointer.any_click()
                && i.pointer
                    .interact_pos()
                    .map(|p| !area.response.rect.contains(p) && !rect.contains(p))
                    .unwrap_or(false)
        });
        if outside_click {
            close_requested = true;
        }
    }

    if close_requested {
        ui.memory_mut(|m| m.close_popup());
    }
    picked
}

/// Paint a single selectable row inside a [`popup_combo`] body. Handles
/// the row hit-rect, the selection / hover state-layer fill, and lets
/// the caller paint the actual row content via `paint`. Returns `true`
/// when the user clicked the row (the caller should stop iterating and
/// return the corresponding value to `popup_combo`).
fn popup_combo_row(
    ui: &mut egui::Ui,
    height: f32,
    selected: bool,
    paint: impl FnOnce(&mut egui::Ui, &egui::Painter, egui::Rect),
) -> bool {
    let resp = ui.allocate_response(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    let r = resp.rect;
    let p = ui.painter().clone();

    let bg = if selected {
        let alpha = if theme::is_dark() { 0.14 } else { 0.24 };
        theme::with_alpha(color::primary(), alpha)
    } else if resp.hovered() {
        let alpha = if theme::is_dark() { 0.06 } else { 0.10 };
        theme::with_alpha(color::on_surface(), alpha)
    } else {
        egui::Color32::TRANSPARENT
    };
    p.rect_filled(r, egui::Rounding::same(radius::XS), bg);

    let inner = r.shrink2(egui::vec2(8.0, 0.0));
    paint(ui, &p, inner);

    resp.clicked()
}

/// Custom ComboBox replacement that paints a real rounded-pill chip next
/// to each config name. Built on top of [`popup_combo`].
fn config_combo(ui: &mut egui::Ui, app: &mut App, width: f32) {
    let configs = &app.configs;
    let current_selected = app.settings.selected_config.as_deref();
    let selected_summary = app
        .selected_entry()
        .map(|e| (e.metadata.name.as_str(), e.metadata.source.kind_label()));

    let picked = popup_combo(
        ui,
        "config_combo_v3",
        width,
        36.0,
        |ui, painter, inner, content_right| {
            // Closed-combo content: name + chip OR placeholder.
            let name_font = egui::FontId::proportional(14.0);
            let center_y = inner.center().y;
            if let Some((name, kind)) = &selected_summary {
                let name_galley = ui.fonts(|f| {
                    f.layout_no_wrap((*name).to_owned(), name_font, color::on_surface())
                });
                let ns = name_galley.size();
                painter.galley(
                    egui::pos2(inner.left(), center_y - ns.y * 0.5),
                    name_galley,
                    color::on_surface(),
                );
                let cs = theme::chip_size(ui, kind);
                let chip_left = inner.left() + ns.x + 8.0;
                if chip_left + cs.x <= content_right {
                    let chip_center = egui::pos2(chip_left + cs.x * 0.5, center_y);
                    let _ = theme::paint_chip(ui, painter, chip_center, kind);
                }
            } else {
                let placeholder = ui.fonts(|f| {
                    f.layout_no_wrap("(none)".into(), name_font, color::on_surface_variant())
                });
                let ps = placeholder.size();
                painter.galley(
                    egui::pos2(inner.left(), center_y - ps.y * 0.5),
                    placeholder,
                    color::on_surface_variant(),
                );
            }
        },
        |ui| {
            if configs.is_empty() {
                ui.label(
                    egui::RichText::new("(no configs — click + Add)")
                        .color(color::on_surface_variant()),
                );
                return None;
            }
            for config in configs {
                let slug = config.slug.as_str();
                let name = config.metadata.name.as_str();
                let kind = config.metadata.source.kind_label();
                let selected = current_selected == Some(slug);
                let clicked = popup_combo_row(ui, 32.0, selected, |ui, p, inner| {
                    let cy = inner.center().y;
                    let name_galley = ui.fonts(|f| {
                        f.layout_no_wrap(
                            name.to_owned(),
                            egui::FontId::proportional(14.0),
                            color::on_surface(),
                        )
                    });
                    let ns = name_galley.size();
                    p.galley(
                        egui::pos2(inner.left(), cy - ns.y * 0.5),
                        name_galley,
                        color::on_surface(),
                    );
                    let cs = theme::chip_size(ui, kind);
                    let chip_center = egui::pos2(inner.left() + ns.x + 8.0 + cs.x * 0.5, cy);
                    let _ = theme::paint_chip(ui, p, chip_center, kind);
                });
                if clicked {
                    return Some(slug.to_owned());
                }
            }
            None
        },
    );

    if let Some(sel) = picked {
        if app.settings.selected_config.as_deref() != Some(sel.as_str()) {
            app.settings.selected_config = Some(sel);
            app.persist_settings();
        }
    }
}

/// Hand-painted core selector — same combo language as `config_combo` so
/// the Settings card and the Configs card read as one design system.
fn core_combo(ui: &mut egui::Ui, app: &mut App, width: f32) {
    let cores = &app.cores;
    let current = app.settings.selected_core.as_deref();

    let picked = popup_combo(
        ui,
        "core_combo_v1",
        width,
        32.0,
        |ui, painter, inner, _content_right| {
            let name_font = egui::FontId::proportional(13.5);
            let (text, color_) = match current {
                Some(name) => (name.to_string(), color::on_surface()),
                None => ("(none)".to_string(), color::on_surface_variant()),
            };
            let galley = ui.fonts(|f| f.layout_no_wrap(text, name_font, color_));
            let center_y = inner.center().y;
            painter.galley(
                egui::pos2(inner.left(), center_y - galley.size().y * 0.5),
                galley,
                color_,
            );
        },
        |ui| {
            if cores.is_empty() {
                ui.label(
                    egui::RichText::new("(no *.exe under core/)")
                        .color(color::on_surface_variant()),
                );
                return None;
            }
            for name in cores {
                let selected = current == Some(name.as_str());
                let clicked = popup_combo_row(ui, 28.0, selected, |ui, p, inner| {
                    let cy = inner.center().y;
                    let g = ui.fonts(|f| {
                        f.layout_no_wrap(
                            name.to_owned(),
                            egui::FontId::proportional(13.5),
                            color::on_surface(),
                        )
                    });
                    p.galley(
                        egui::pos2(inner.left(), cy - g.size().y * 0.5),
                        g,
                        color::on_surface(),
                    );
                });
                if clicked {
                    return Some(Some(name.to_owned()));
                }
            }
            None
        },
    );

    if let Some(sel) = picked {
        if app.settings.selected_core != sel {
            app.settings.selected_core = sel;
            app.persist_settings();
        }
    }
}

fn inbound_override_settings(ui: &mut egui::Ui, app: &mut App) {
    let mut changed = false;
    if theme::switch(
        ui,
        &mut app.settings.inbound_override.enabled,
        "Override inbound configuration",
    )
    .changed()
    {
        changed = true;
    }

    if app.settings.inbound_override.enabled {
        settings_child_row(ui, |ui| {
            ui.label(theme::setting_label("Override inbound with"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if inbound_override_kind_combo(ui, &mut app.settings.inbound_override.kind, 132.0)
                    .is_some()
                {
                    changed = true;
                }
            });
        });

        match app.settings.inbound_override.kind {
            InboundOverrideKind::MixedIn => {
                settings_child_row(ui, |ui| {
                    ui.label(theme::setting_label("Listen address"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::setting_input_singleline_sized(
                            ui,
                            &mut app.settings.inbound_override.mixed_listen,
                            DEFAULT_MIXED_LISTEN,
                            SETTINGS_INPUT_ADDRESS,
                        )
                        .changed()
                        {
                            changed = true;
                        }
                    });
                });
                settings_child_row(ui, |ui| {
                    ui.label(theme::setting_label("Listen port"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let port_hint = DEFAULT_MIXED_LISTEN_PORT.to_string();
                        if theme::setting_input_singleline_sized(
                            ui,
                            &mut app.settings.inbound_override.mixed_listen_port,
                            &port_hint,
                            SETTINGS_INPUT_SHORT,
                        )
                        .changed()
                        {
                            changed = true;
                        }
                    });
                });
            }
            InboundOverrideKind::Tun => {
                settings_child_row(ui, |ui| {
                    ui.label(theme::setting_label("MTU"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::setting_input_singleline_sized(
                            ui,
                            &mut app.settings.inbound_override.tun_mtu,
                            "default",
                            SETTINGS_INPUT_MEDIUM,
                        )
                        .changed()
                        {
                            changed = true;
                        }
                    });
                });
                settings_child_row(ui, |ui| {
                    if theme::switch(
                        ui,
                        &mut app.settings.inbound_override.tun_endpoint_independent_nat,
                        "Endpoint independent NAT",
                    )
                    .changed()
                    {
                        changed = true;
                    }
                });
            }
        }
    }

    if changed {
        app.persist_settings();
    }
}

fn inbound_override_kind_combo(
    ui: &mut egui::Ui,
    value: &mut InboundOverrideKind,
    width: f32,
) -> Option<InboundOverrideKind> {
    let current = *value;
    let picked = popup_combo(
        ui,
        "inbound_override_kind_combo",
        width,
        28.0,
        |ui, painter, inner, _content_right| {
            let text = inbound_override_kind_label(current);
            let galley = ui.fonts(|f| {
                f.layout_no_wrap(
                    text.to_owned(),
                    egui::FontId::proportional(13.0),
                    color::on_surface(),
                )
            });
            painter.galley(
                egui::pos2(inner.left(), inner.center().y - galley.size().y * 0.5),
                galley,
                color::on_surface(),
            );
        },
        |ui| {
            for option in [InboundOverrideKind::MixedIn, InboundOverrideKind::Tun] {
                let selected = current == option;
                let clicked = popup_combo_row(ui, 28.0, selected, |ui, painter, inner| {
                    let text = inbound_override_kind_label(option);
                    let galley = ui.fonts(|f| {
                        f.layout_no_wrap(
                            text.to_owned(),
                            egui::FontId::proportional(13.0),
                            color::on_surface(),
                        )
                    });
                    painter.galley(
                        egui::pos2(inner.left(), inner.center().y - galley.size().y * 0.5),
                        galley,
                        color::on_surface(),
                    );
                });
                if clicked {
                    return Some(option);
                }
            }
            None
        },
    );
    if let Some(picked) = picked {
        if *value != picked {
            *value = picked;
            return Some(picked);
        }
    }
    None
}

fn inbound_override_kind_label(kind: InboundOverrideKind) -> &'static str {
    match kind {
        InboundOverrideKind::MixedIn => "mixedin",
        InboundOverrideKind::Tun => "tun",
    }
}

fn log_override_settings(ui: &mut egui::Ui, app: &mut App) {
    let mut changed = false;
    if theme::switch(
        ui,
        &mut app.settings.log_override.enabled,
        "Override log configuration",
    )
    .changed()
    {
        changed = true;
    }

    if app.settings.log_override.enabled {
        settings_child_row(ui, |ui| {
            if theme::switch(ui, &mut app.settings.log_override.disabled, "Disabled").changed() {
                changed = true;
            }
        });
        settings_child_row(ui, |ui| {
            ui.label(theme::setting_label("Level"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if log_level_combo(ui, &mut app.settings.log_override.level, 132.0).is_some() {
                    changed = true;
                }
            });
        });
        settings_child_row(ui, |ui| {
            if theme::switch(ui, &mut app.settings.log_override.save_logs, "Save logs").changed() {
                changed = true;
            }
        });
    }

    if changed {
        app.persist_settings();
    }
}

fn log_level_combo(ui: &mut egui::Ui, value: &mut LogLevel, width: f32) -> Option<LogLevel> {
    let current = *value;
    let picked = popup_combo(
        ui,
        "log_level_combo",
        width,
        28.0,
        |ui, painter, inner, _content_right| {
            let text = current.as_str();
            let galley = ui.fonts(|f| {
                f.layout_no_wrap(
                    text.to_owned(),
                    egui::FontId::proportional(13.0),
                    color::on_surface(),
                )
            });
            painter.galley(
                egui::pos2(inner.left(), inner.center().y - galley.size().y * 0.5),
                galley,
                color::on_surface(),
            );
        },
        |ui| {
            for option in [
                LogLevel::Trace,
                LogLevel::Debug,
                LogLevel::Info,
                LogLevel::Warn,
                LogLevel::Error,
                LogLevel::Fatal,
                LogLevel::Panic,
            ] {
                let selected = current == option;
                let clicked = popup_combo_row(ui, 28.0, selected, |ui, painter, inner| {
                    let text = option.as_str();
                    let galley = ui.fonts(|f| {
                        f.layout_no_wrap(
                            text.to_owned(),
                            egui::FontId::proportional(13.0),
                            color::on_surface(),
                        )
                    });
                    painter.galley(
                        egui::pos2(inner.left(), inner.center().y - galley.size().y * 0.5),
                        galley,
                        color::on_surface(),
                    );
                });
                if clicked {
                    return Some(option);
                }
            }
            None
        },
    );
    if let Some(picked) = picked {
        if *value != picked {
            *value = picked;
            return Some(picked);
        }
    }
    None
}

fn detail_value(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(color::on_surface()));
}

// ---------- Settings card ----------

/// The two admin-mode toggles rendered inside the APP group.
///
/// Design rationale & Windows constraints:
///
/// * Windows has no API to elevate or de-elevate a *running* process —
///   the only way to change a process's integrity level is to spawn a
///   new process with a different token. So the **first toggle**
///   (current-session admin mode) inherently requires a restart on
///   either direction:
///     * OFF → ON: `ShellExecuteExW("runas")` triggers a UAC prompt;
///       on consent the new elevated instance starts and we exit.
///     * ON → OFF: we duplicate `explorer.exe`'s medium-integrity
///       primary token and `CreateProcessWithTokenW` ourselves with
///       it (no UAC). When that succeeds we exit; the new
///       standard-user instance takes over.
///
/// * The **second toggle** (persistent admin) only mutates a stored
///   setting and creates/removes a Task Scheduler task — no restart
///   is needed. It only makes sense (and is only shown) once the
///   first toggle is on AND the process is actually elevated, since
///   registering a `RunLevel=HighestAvailable` task itself requires
///   admin.
///
/// The first toggle's bound `&mut bool` is a local mirror of
/// `is_elevated()`: it is never persisted, just reflects the current
/// process token. If a restart fails (e.g. UAC denied), the next
/// frame redraws with the unchanged real value, so the toggle
/// visually springs back on its own.
fn admin_mode_toggles(ui: &mut egui::Ui, app: &mut App) {
    use crate::core::elevation;

    let elevated = elevation::is_elevated();

    // -- First toggle: session-level Administrator Mode.
    let mut admin_on = elevated;
    let resp = theme::switch(
        ui,
        &mut admin_on,
        "Administrator Mode (Required when using TUN)",
    );
    if resp.changed() {
        if admin_on {
            // OFF → ON: relaunch as administrator (UAC prompt).
            app.try_stop();
            match elevation::restart_as_admin(&app.paths.root) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        app.last_error = Some(
                            "Elevation cancelled. Click Yes on the UAC \
                             prompt to enable Administrator Mode."
                                .into(),
                        );
                    } else {
                        app.last_error = Some(format!("Restart as admin failed: {e}"));
                    }
                }
            }
        } else {
            // ON → OFF: relaunch as standard user. Clear `always_admin`
            // first so the new standard-user instance doesn't get
            // immediately re-promoted by the schtasks /run handshake
            // in main().
            if app.settings.always_admin {
                app.settings.always_admin = false;
                app.persist_settings();
            }
            app.try_stop();
            match elevation::restart_as_standard_user(&app.paths.root) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    app.last_error = Some(format!("Restart as standard user failed: {e}"));
                }
            }
        }
    }

    // -- Second toggle: persistence (only visible when actually elevated).
    if elevated {
        let prev = app.settings.always_admin;
        settings_child_row(ui, |ui| {
            let resp = theme::switch(
                ui,
                &mut app.settings.always_admin,
                "Always enable administrator mode",
            );
            if resp.changed() {
                let now = app.settings.always_admin;
                app.persist_settings();
                if now && !prev {
                    app.last_info = Some(
                        "Persistent admin enabled. Future launches will start as \
                         Administrator with no UAC prompt."
                            .into(),
                    );
                } else if !now && prev {
                    app.last_info = Some(
                        "Persistent admin disabled. Future launches will run as \
                         standard user."
                            .into(),
                    );
                }
            }
        });
    }
}

fn settings_card(ui: &mut egui::Ui, app: &mut App, card_width: f32) {
    theme::card_with_width(ui, card_width, |ui| {
        theme::section_title(ui, "Settings");
        ui.add_space(8.0);

        theme::subsection_title(ui, "APP");
        ui.add_space(2.0);
        // Plain vertical group (no `ui.indent` — it paints a left guide line
        // we don't want; the subsection title already conveys grouping).
        ui.vertical(|ui| {
            // Theme picker — first option in the card so the user can
            // immediately re-skin everything else they're about to read.
            // Layout matches the switch rows below: label flush-left,
            // small pill flush-right.
            settings_row(ui, |ui| {
                ui.label(theme::setting_label("Theme"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::segmented(
                        ui,
                        "theme_mode_picker",
                        &mut app.settings.theme_mode,
                        &[
                            (theme::ThemeMode::Dark, "Dark"),
                            (theme::ThemeMode::Light, "Light"),
                            (theme::ThemeMode::System, "System"),
                        ],
                    )
                    .changed()
                    {
                        app.persist_settings();
                    }
                });
            });

            // Admin-mode toggles come next because they're the most
            // consequential setting (the only one that requires a UAC
            // prompt and a process restart to take effect).
            admin_mode_toggles(ui, app);

            let mut s_changed = false;
            if theme::switch(
                ui,
                &mut app.settings.auto_start,
                "Launch on Windows startup",
            )
            .changed()
            {
                s_changed = true;
            }
            if theme::switch(
                ui,
                &mut app.settings.close_to_tray,
                "Close button hides to tray",
            )
            .changed()
            {
                s_changed = true;
            }
            if theme::switch(ui, &mut app.settings.silent_start, "Silent startup").changed() {
                s_changed = true;
            }
            if theme::switch(
                ui,
                &mut app.settings.auto_update,
                "Auto-update remote configs",
            )
            .changed()
            {
                s_changed = true;
            }
            if app.settings.auto_update {
                settings_child_row(ui, |ui| {
                    ui.label(theme::setting_label("Auto update interval (hours)"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let interval_hint = DEFAULT_UPDATE_INTERVAL_HOURS.to_string();
                        let resp = theme::setting_input_singleline_sized(
                            ui,
                            &mut app.update_interval_input,
                            &interval_hint,
                            SETTINGS_INPUT_SHORT,
                        );
                        if resp.changed() {
                            let value = app.update_interval_input.trim();
                            let parsed = if value.is_empty() {
                                Some(DEFAULT_UPDATE_INTERVAL_HOURS)
                            } else {
                                value.parse::<u64>().ok().map(|hours| hours.clamp(1, 720))
                            };
                            if let Some(hours) = parsed {
                                if app.settings.update_interval_hours != hours {
                                    app.settings.update_interval_hours = hours;
                                    s_changed = true;
                                }
                            }
                        }
                        if resp.lost_focus() {
                            app.update_interval_input = if app.settings.update_interval_hours
                                == DEFAULT_UPDATE_INTERVAL_HOURS
                            {
                                String::new()
                            } else {
                                app.settings.update_interval_hours.to_string()
                            };
                        }
                    });
                });
            }
            if s_changed {
                app.persist_settings();
            }

            // ----- AppContainer loopback utility -----
            // Surfaced here because, like the other APP toggles, it
            // affects how Windows treats this proxy launcher's traffic
            // scope (UWP / Edge / Store apps need an exemption to
            // reach a local proxy at all). The button hands off to a
            // background worker that checks the well-known install
            // path, downloads Telerik's installer if needed, then
            // shells out to whichever copy is on disk \u2014 the binary
            // carries its own UAC manifest so a prompt fires.
            //
            // Network I/O lives on the worker thread; this row only
            // sends a `BgCmd::OpenLoopback` and disables the button
            // until the corresponding `BgEvent::LoopbackDone` lands
            // (`app.loopback_busy`).
            #[cfg(windows)]
            {
                ui.add_space(2.0);
                settings_row_sized(ui, SETTINGS_ACTION_ROW_HEIGHT, |ui| {
                    let label = ui.label(theme::setting_label(crate::core::loopback::DISPLAY_NAME));
                    label.on_hover_text(crate::core::loopback::HOVER_TEXT);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Keep this as a compact, low-emphasis row action
                        // rather than a full tonal pill.
                        let label = if app.loopback_busy {
                            "Opening\u{2026}"
                        } else {
                            "Open"
                        };
                        let loopback_button = egui::Button::new(
                            egui::RichText::new(label)
                                .color(color::on_surface())
                                .size(12.0),
                        )
                        .fill(color::surface_container_high())
                        .rounding(egui::Rounding::same(radius::FULL))
                        .min_size(egui::Vec2::new(56.0, 22.0))
                        .stroke(egui::Stroke::new(1.0, color::outline_variant()));
                        let resp = ui.add(loopback_button).on_hover_text(
                            "Launch EnableLoopback.exe (downloads from Telerik on first use; UAC \
                             will prompt)",
                        );
                        if resp.clicked() && !app.loopback_busy {
                            app.loopback_busy = true;
                            app.last_error = None;
                            app.last_info = Some(format!(
                                "Preparing {}\u{2026}",
                                crate::core::loopback::DISPLAY_NAME
                            ));
                            let _ = app.bg_tx.send(BgCmd::OpenLoopback);
                        }
                    });
                });
            }
        });

        ui.add_space(12.0);
        subtle_divider(ui);
        ui.add_space(10.0);
        theme::subsection_title(ui, "SING-BOX");
        ui.add_space(2.0);
        ui.vertical(|ui| {
            if theme::switch(
                ui,
                &mut app.settings.auto_start_sing_box,
                "Start sing-box on app launch",
            )
            .changed()
            {
                app.persist_settings();
            }

            log_override_settings(ui, app);
            inbound_override_settings(ui, app);

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(theme::setting_label("Core"));
                let avail = ui.available_width();
                // Reserve room on the right for the two icon buttons.
                let combo_w = (avail - 2.0 * 32.0 - 2.0 * ui.spacing().item_spacing.x).max(120.0);
                core_combo(ui, app, combo_w);
                if theme::folder_button(ui)
                    .on_hover_text("Open core folder")
                    .clicked()
                {
                    if let Err(e) = shell::open(&app.paths.core_dir) {
                        app.last_error = Some(e.to_string());
                    }
                }
                if theme::refresh_button(ui)
                    .on_hover_text("Refresh core list")
                    .clicked()
                {
                    if let Some(e) = app.refresh_listings() {
                        app.last_error = Some(e);
                    }
                }
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                external_link_button(
                    ui,
                    app,
                    "sing-box GitHub",
                    "https://github.com/SagerNet/sing-box",
                );
                external_link_button(ui, app, "sing-box Docs", "https://sing-box.sagernet.org/");
                if ui.add(theme::tonal_button("Open Working Dir")).clicked() {
                    if let Err(e) = shell::open(&app.paths.working_dir) {
                        app.last_error = Some(e.to_string());
                    }
                }
                if ui
                    .add(theme::destructive_button("Destroy Working Dir"))
                    .clicked()
                {
                    app.destroy_confirm_open = true;
                }
            });
        });

        // ---------- ABOUT ----------
        ui.add_space(12.0);
        subtle_divider(ui);
        ui.add_space(10.0);
        theme::subsection_title(ui, "ABOUT");
        ui.add_space(2.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(theme::setting_label("Version"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(env!("CARGO_PKG_VERSION")).color(color::on_surface()),
                    );
                });
            });
            ui.horizontal(|ui| {
                ui.label(theme::setting_label("Copyright"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("© 2026 ashton2914").color(color::on_surface()));
                });
            });
            ui.horizontal(|ui| {
                ui.label(theme::setting_label("License"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            theme::tonal_button("View license & disclaimer")
                                .min_size(egui::Vec2::new(180.0, 28.0)),
                        )
                        .clicked()
                    {
                        app.about_modal_state.reset();
                        app.about_open = true;
                    }
                });
            });
        });
    });
}

fn external_link_button(ui: &mut egui::Ui, app: &mut App, label: &'static str, url: &'static str) {
    if ui
        .add(theme::tonal_button(label))
        .on_hover_text(format!("Open {url}"))
        .clicked()
    {
        if let Err(e) = shell::open_url(url) {
            app.last_error = Some(format!("Open {label} failed: {e}"));
        }
    }
}

fn subtle_divider(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, egui::Rounding::ZERO, color::outline_variant());
}

// ---------- Run / status / logs ----------

/// Small pill chip showing the current process integrity. Painted
/// inline (no extra row of vertical space) and color-coded so a glance
/// at the top card tells the user whether TUN mode will work:
/// * Administrator → success-tinted pill
/// * Standard user → tertiary-tinted pill (the same neutral hue used
///   for the not-yet-elevated shield in the Settings card)
fn elevation_pill(ui: &mut egui::Ui) {
    use crate::core::elevation;

    let elevated = elevation::is_elevated();
    let (label, fg, bg) = if elevated {
        (
            "ADMIN",
            color::on_primary_container(),
            theme::with_alpha(color::success(), 0.22),
        )
    } else {
        (
            "USER",
            color::on_surface_variant(),
            color::surface_container_highest(),
        )
    };

    let font = egui::FontId::proportional(10.5);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.into(), font, fg));
    let pad_x = 8.0;
    let pad_y = 3.0;
    let size = galley.size() + egui::vec2(pad_x * 2.0, pad_y * 2.0);

    let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    let rounding = egui::Rounding::same(rect.height() * 0.5);
    painter.rect_filled(rect, rounding, bg);
    if elevated {
        painter.rect_stroke(
            rect,
            rounding,
            egui::Stroke::new(1.0, theme::with_alpha(color::success(), 0.55)),
        );
    } else {
        painter.rect_stroke(
            rect,
            rounding,
            egui::Stroke::new(1.0, color::outline_variant()),
        );
    }
    painter.galley(rect.left_top() + egui::vec2(pad_x, pad_y), galley, fg);
}

fn run_card(ui: &mut egui::Ui, app: &mut App, card_width: f32) {
    let running = app.proc.is_running();
    // Keep the running-uptime label ticking once per second — but only
    // while the window is actually on screen. When the user has hidden
    // the app to the tray (silent_start or close_to_tray), winit still
    // delivers `RedrawRequested` for every queued `request_repaint*`, so
    // an unconditional 1s self-loop here burns a full layout pass per
    // second on an invisible UI. Gating on `main_window_visible()`
    // stops the loop dead while in the tray; reopening the window the
    // next `update()` re-arms it.
    if running && app.main_window_visible() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));
    }

    theme::card_with_width(ui, card_width, |ui| {
        ui.horizontal(|ui| {
            ui.set_height(44.0);
            let (icon, hover) = if running {
                (theme::FabIcon::Stop, "Stop sing-box")
            } else {
                (theme::FabIcon::Play, "Start sing-box")
            };
            if theme::fab_button(ui, icon).on_hover_text(hover).clicked() {
                if running {
                    app.try_stop();
                } else {
                    app.try_start();
                }
            }
            ui.add_space(12.0);
            let (status_color, status_text) = if running {
                (color::success(), "Running")
            } else {
                (color::on_surface_variant(), "Stopped")
            };
            status_text_block(ui, status_text, status_color);
            // Right-align the process integrity chip on the status row.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                elevation_pill(ui);
            });
        });

        if let Some(err) = app.last_error.clone() {
            ui.add_space(8.0);
            if banner(ui, &err, color::error_container(), color::error()) {
                app.last_error = None;
            }
        } else if let Some(info) = app.last_info.clone() {
            // Success / informational banner — same shape as the error
            // banner but tinted with the success palette. Previously
            // `last_info` was set on Add/Update/Delete success but never
            // rendered, so the user got no visible feedback for those
            // actions outside the log card.
            ui.add_space(8.0);
            if banner(
                ui,
                &info,
                theme::with_alpha(color::success(), 0.18),
                color::on_surface(),
            ) {
                app.last_info = None;
            }
        }

        ui.add_space(14.0);

        const LOG_INNER_MARGIN: f32 = 10.0;
        const LOG_HEIGHT: f32 = 200.0;
        const LOG_TOOLBAR_HEIGHT: f32 = 28.0;
        const LOG_DIVIDER_HEIGHT: f32 = 1.0;
        const LOG_BODY_HEIGHT: f32 = LOG_HEIGHT - LOG_TOOLBAR_HEIGHT - LOG_DIVIDER_HEIGHT;
        let log_inner_width = (ui.available_width() - LOG_INNER_MARGIN * 2.0).max(0.0);

        let log_frame = egui::Frame::none()
            .fill(color::surface_container_lowest())
            .rounding(egui::Rounding::same(radius::MD))
            .inner_margin(egui::Margin::same(LOG_INNER_MARGIN))
            .show(ui, |ui| {
                ui.set_min_width(log_inner_width);
                ui.set_max_width(log_inner_width);
                ui.set_min_height(LOG_HEIGHT);

                // The log view has dense monospace text. Keep the scrollbar
                // permanently visible (no auto-hide) and reserve a gutter so
                // it never overlaps text. Also force background-color mode:
                // the default floating style uses foreground text color for
                // handles, which stays bright even after `floating = false`.
                {
                    let (inactive, hovered, active) = if theme::is_dark() {
                        (
                            theme::with_alpha(color::outline_variant(), 0.55),
                            theme::with_alpha(color::outline_variant(), 0.85),
                            color::outline_variant(),
                        )
                    } else {
                        (
                            egui::Color32::from_rgb(214, 214, 218),
                            egui::Color32::from_rgb(198, 198, 204),
                            egui::Color32::from_rgb(180, 180, 188),
                        )
                    };
                    let visuals = &mut ui.visuals_mut().widgets;
                    visuals.inactive.bg_fill = inactive;
                    visuals.hovered.bg_fill = hovered;
                    visuals.active.bg_fill = active;
                }
                {
                    let scroll = &mut ui.spacing_mut().scroll;
                    scroll.floating = false;
                    scroll.foreground_color = false;
                    scroll.bar_width = 8.0;
                    scroll.bar_inner_margin = 2.0;
                    scroll.bar_outer_margin = 0.0;
                }

                ui.allocate_ui_with_layout(
                    egui::vec2(log_inner_width, LOG_TOOLBAR_HEIGHT),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        log_status_toolbar(ui, app);
                    },
                );
                log_toolbar_divider(ui, log_inner_width);

                egui::ScrollArea::both()
                    .auto_shrink([false; 2])
                    .drag_to_scroll(false)
                    .max_height(LOG_BODY_HEIGHT)
                    .stick_to_bottom(app.log_view_snapshot.is_none())
                    .show(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        // Disable text selection entirely in the log body —
                        // labels render as non-interactive so dragging never
                        // starts a selection.
                        ui.style_mut().interaction.selectable_labels = false;
                        let snapshot = app.log_view_snapshot.as_deref();
                        let visible_empty = snapshot
                            .map(|s| s.is_empty())
                            .unwrap_or(app.logs.is_empty());
                        if visible_empty {
                            ui.label(
                                egui::RichText::new("(no logs yet)")
                                    .color(color::on_surface_variant()),
                            );
                        }
                        let render_entry = |ui: &mut egui::Ui, log: &LogEvent| {
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&log.timestamp)
                                            .color(color::on_surface_variant())
                                            .monospace(),
                                    )
                                    .selectable(false),
                                );
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&log.message)
                                            .color(color::on_surface())
                                            .monospace(),
                                    )
                                    .selectable(false),
                                );
                            });
                        };
                        if let Some(snapshot) = snapshot {
                            for log in snapshot {
                                render_entry(ui, log);
                            }
                        } else {
                            for log in &app.logs {
                                render_entry(ui, log);
                            }
                        }
                    });
            });
        ui.ctx().data_mut(|data| {
            data.insert_temp(log_scroll_blocking_rect_id(), log_frame.response.rect);
        });
    });
}

fn log_status_toolbar(ui: &mut egui::Ui, app: &mut App) {
    // Refresh the cached `<core> version` output if the user swapped
    // cores or replaced the binary on disk. Cheap when the cache is
    // valid; spawns the core process once otherwise.
    app.ensure_core_version();
    // Probe the monospace font's design row once per frame; every
    // chip/label below shares these metrics so they all sit on a
    // single visual baseline.
    let chips = LogInlineChips::new(ui);
    let uptime = app
        .proc
        .running_for()
        .map(format_duration)
        .unwrap_or_else(|| "stopped".to_owned());

    // Snapshot the log-override fields before reading the cached config
    // so the immutable copy here doesn't conflict with the `&mut app`
    // borrow that `selected_config_value` needs.
    let lo_enabled = app.settings.log_override.enabled;
    let lo_disabled = app.settings.log_override.disabled;
    let lo_level = app.settings.log_override.level;
    let cfg = app.selected_config_value();
    let log_level = if lo_enabled {
        if lo_disabled {
            "disabled".to_owned()
        } else {
            lo_level.as_str().to_owned()
        }
    } else {
        cfg.and_then(extract_log_level_from_config)
            .unwrap_or_else(|| "info".to_owned())
    };
    let webui_url = cfg.and_then(extract_clash_webui_url);

    // ---- Core: <short version> ----
    // Hidden-button styling: looks like plain status text until the
    // user hovers, at which point a subtle background hints that
    // clicking opens the full `<core> version` output in a modal.
    let core_short = app
        .core_version
        .short
        .clone()
        .unwrap_or_else(|| "?".to_owned());
    let core_has_full = app.core_version.full.is_some();
    let core_label = format!("Core: {core_short}");
    if chips
        .button(ui, &core_label, color::on_surface_variant())
        .clicked()
        && core_has_full
    {
        app.core_version_modal_state.reset();
        app.core_version_open = true;
    }

    ui.add_space(8.0);

    // Log: <level> + Uptime: <duration> as a single static label —
    // these are read-only status text, not interactive. Time-of-day is
    // intentionally absent here; the OS clock already covers that and
    // it added noise. "Log:" rather than "Level" because the value is
    // the log subsystem state (which includes the literal "disabled")
    // and "Level" misleadingly implied a strict severity threshold.
    //
    // Rendered via `chips.text` (rather than `egui::Label`) so it
    // shares the same baseline math as the hidden-button chips on either
    // side. Otherwise strings without descenders ("WebUI", "Clear",
    // "Pause") would visibly sit at a different height than this label.
    let text = format!("Log: {log_level}    Uptime: {uptime}");
    chips.text(ui, &text, color::on_surface_variant());

    if let Some(url) = webui_url.as_deref() {
        ui.add_space(8.0);
        // WebUI affordance uses the same muted color as surrounding
        // status text so it does not visually compete with the other
        // chips — only the hover background reveals it is clickable.
        if chips
            .button(ui, "WebUI", color::on_surface_variant())
            .clicked()
        {
            if let Err(e) = shell::open_url(url) {
                app.last_error = Some(format!("Open WebUI failed: {e}"));
            }
        }
    }

    // Right-aligned action chips: Pause/Resume and Clear share the same
    // hidden-button chrome as the left-side chips but keep the primary
    // accent color so the user can still pick them out as actions.
    // Both are always visible (Clear is a no-op when the log is empty)
    // so the toolbar layout never shifts when logs arrive or are cleared.
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let paused = app.log_view_snapshot.is_some();
        let label = if paused { "Resume" } else { "Pause" };
        if chips.button(ui, label, color::primary()).clicked() {
            if paused {
                app.log_view_snapshot = None;
            } else {
                app.log_view_snapshot = Some(app.logs.iter().cloned().collect());
            }
        }
        if chips.button(ui, "Clear", color::primary()).clicked() {
            app.logs.clear();
            app.log_view_snapshot = None;
        }
    });
}

/// Horizontal/vertical padding around the typographic design row when
/// painting a chip background. Vertical padding leaves a small breathing
/// gap above the caps and below the descenders so the chip doesn't read
/// as cramped, while still hugging the font's design row more tightly
/// than `egui::Button` would.
const LOG_INLINE_PAD: egui::Vec2 = egui::vec2(6.0, 4.0);

/// Renders the monospace status text and hidden-button chips that make
/// up the log toolbar. Constructed once per toolbar frame so the font's
/// typographic design row — probed once by laying out a reference
/// string containing both an ascender ("A") and a descender ("g") — is
/// shared by every label.
///
/// Using these reference metrics (rather than each string's
/// `mesh_bounds`) is what keeps adjacent labels on the same baseline
/// regardless of which glyphs they happen to contain. Per-string
/// `mesh_bounds` would shift the cap-top up for strings like "WebUI"
/// (no descender), breaking baseline alignment with the rest of the
/// toolbar.
struct LogInlineChips {
    font_id: egui::FontId,
    /// Cap-top to descender-bottom extent of the reference glyphs, in
    /// galley units. Used as every chip's content height so the chip
    /// background hugs the design row, not the per-string ink.
    design_height: f32,
    /// Y offset of the cap-top inside the reference galley's layout
    /// box. Used to anchor each label's cap-top at a fixed position
    /// inside its chip, which is what produces the unified baseline.
    cap_top_in_galley: f32,
}

impl LogInlineChips {
    fn new(ui: &egui::Ui) -> Self {
        let font_id = egui::FontId::monospace(12.0);
        let reference =
            ui.fonts(|f| f.layout_no_wrap("Ag".to_owned(), font_id.clone(), egui::Color32::WHITE));
        Self {
            font_id,
            design_height: reference.mesh_bounds.height(),
            cap_top_in_galley: reference.mesh_bounds.min.y,
        }
    }

    /// Paint a non-interactive monospace label using the exact same
    /// vertical positioning math as the hidden-button chips. This is
    /// what guarantees the plain status text ("Log: warn    Uptime:
    /// stopped") sits on the same baseline as the adjacent chips, even
    /// when the chips contain strings without descenders.
    fn text(&self, ui: &mut egui::Ui, text: &str, color: egui::Color32) {
        self.paint(ui, text, color, false);
    }

    /// Render a button that looks like the surrounding monospace status
    /// text until the pointer hovers over it. On hover/active, a subtle
    /// surface tint paints behind the label so the affordance becomes
    /// visible without crowding the toolbar with chrome.
    ///
    /// Painted by hand so every chip's hover background hugs the
    /// font's typographic design row (cap-top to descender-bottom)
    /// rather than the per-string ink, and so every label — chip or
    /// not — sits on the same baseline.
    fn button(&self, ui: &mut egui::Ui, text: &str, color: egui::Color32) -> egui::Response {
        self.paint(ui, text, color, true)
    }

    fn paint(
        &self,
        ui: &mut egui::Ui,
        text: &str,
        color: egui::Color32,
        interactive: bool,
    ) -> egui::Response {
        let galley = ui.fonts(|f| f.layout_no_wrap(text.to_owned(), self.font_id.clone(), color));

        // Chip frames the *design row* (consistent across all strings)
        // plus symmetric padding, so every chip has the same height and
        // the hover background lines up with neighbouring chips even
        // when the string has no descender ink to fill the bottom of
        // the row.
        let chip_size = egui::vec2(
            galley.size().x + LOG_INLINE_PAD.x * 2.0,
            self.design_height + LOG_INLINE_PAD.y * 2.0,
        );
        let row_height = ui.available_height().max(chip_size.y);
        let sense = if interactive {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };
        let (slot_rect, response) =
            ui.allocate_exact_size(egui::vec2(chip_size.x, row_height), sense);
        let chip_rect = egui::Rect::from_center_size(slot_rect.center(), chip_size);

        if interactive {
            let bg = if response.is_pointer_button_down_on() {
                theme::with_alpha(color::on_surface(), 0.16)
            } else if response.hovered() {
                theme::with_alpha(color::on_surface(), 0.10)
            } else {
                egui::Color32::TRANSPARENT
            };
            if bg.a() > 0 {
                ui.painter()
                    .rect_filled(chip_rect, egui::Rounding::same(radius::SM), bg);
            }
        }

        // Anchor the font's cap-top (not the galley's layout-box top)
        // to a fixed offset inside the chip. The galley draws relative
        // to its own layout origin, so we shift by `-cap_top_in_galley`
        // to put the actual cap-tops at
        // `chip_rect.top() + LOG_INLINE_PAD.y`. Every call — chip or
        // plain text — uses the same target y, which is what produces
        // the unified baseline.
        let target_cap_top = chip_rect.top() + LOG_INLINE_PAD.y;
        let text_pos = egui::pos2(
            chip_rect.left() + LOG_INLINE_PAD.x,
            target_cap_top - self.cap_top_in_galley,
        );
        ui.painter().galley(text_pos, galley, color);

        response
    }
}

fn extract_log_level_from_config(config: &serde_json::Value) -> Option<String> {
    let log = config.get("log")?;
    if log
        .get("disabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Some("disabled".to_owned());
    }
    log.get("level")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn extract_clash_webui_url(config: &serde_json::Value) -> Option<String> {
    let clash_api = config.get("experimental")?.get("clash_api")?;
    let controller = non_empty_json_str(clash_api.get("external_controller")?)?;
    non_empty_json_str(clash_api.get("external_ui")?)?;
    Some(clash_webui_url(controller))
}

fn non_empty_json_str(value: &serde_json::Value) -> Option<&str> {
    let value = value.as_str()?.trim();
    (!value.is_empty()).then_some(value)
}

fn clash_webui_url(controller: &str) -> String {
    let mut controller = controller.trim().replace("0.0.0.0", "127.0.0.1");
    if !controller.starts_with("http://") && !controller.starts_with("https://") {
        controller = format!("http://{controller}");
    }
    format!("{}/ui/", controller.trim_end_matches('/'))
}

fn log_toolbar_divider(ui: &mut egui::Ui, width: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        egui::Stroke::new(1.0, theme::with_alpha(color::outline_variant(), 0.55)),
    );
}

fn format_duration(duration: std::time::Duration) -> String {
    let total = duration.as_secs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn status_text_block(ui: &mut egui::Ui, status_text: &str, status_color: egui::Color32) {
    let width = 130.0;
    let height = 44.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let caption_font = egui::FontId::proportional(10.5);
    let status_font = egui::FontId::proportional(19.0);
    let caption = ui.fonts(|f| {
        f.layout_no_wrap(
            "STATUS".to_string(),
            caption_font,
            color::on_surface_variant(),
        )
    });
    let status = ui.fonts(|f| f.layout_no_wrap(status_text.to_string(), status_font, status_color));
    let line_gap = 2.0;
    let caption_h = caption.size().y;
    let status_h = status.size().y;
    let total_h = caption_h + line_gap + status_h;
    let y = rect.center().y - total_h * 0.5;
    let painter = ui.painter();
    painter.galley(
        rect.left_top() + egui::vec2(0.0, y - rect.top()),
        caption,
        color::on_surface_variant(),
    );
    painter.galley(
        rect.left_top() + egui::vec2(0.0, y - rect.top() + caption_h + line_gap),
        status,
        status_color,
    );
}

// ---------- Helpers ----------

fn banner(ui: &mut egui::Ui, text: &str, bg: egui::Color32, fg: egui::Color32) -> bool {
    // Match the log card's outer width: both panels sit in the same
    // run_card column, so the banner should always span the full
    // available width regardless of how short the message is.
    // Otherwise short errors render as a narrow blob that visually
    // detaches from the log frame below.
    const HORIZONTAL_PAD: f32 = 12.0;
    const VERTICAL_PAD: f32 = 8.0;
    const CLOSE_SIZE: f32 = 20.0;
    const CLOSE_GAP: f32 = 8.0;

    let width = ui.available_width();
    let label_width = (width - HORIZONTAL_PAD * 2.0 - CLOSE_SIZE - CLOSE_GAP).max(0.0);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui.fonts(|f| f.layout(text.to_owned(), font, fg, label_width));
    let height = galley.size().y + VERTICAL_PAD * 2.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, egui::Rounding::same(radius::MD), bg);
    painter.galley(
        rect.left_top() + egui::vec2(HORIZONTAL_PAD, VERTICAL_PAD),
        galley,
        fg,
    );

    let close_rect = egui::Rect::from_center_size(
        egui::pos2(
            rect.right() - HORIZONTAL_PAD - CLOSE_SIZE * 0.5,
            rect.center().y,
        ),
        egui::Vec2::splat(CLOSE_SIZE),
    );
    let close_resp = ui.interact(
        close_rect,
        response.id.with("banner_close"),
        egui::Sense::click(),
    );
    paint_banner_close_button(ui, close_rect, &close_resp, fg);
    close_resp.on_hover_text("Dismiss").clicked()
}

fn paint_banner_close_button(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    resp: &egui::Response,
    fg: egui::Color32,
) {
    let painter = ui.painter().clone();
    if resp.hovered() {
        painter.circle_filled(
            rect.center(),
            10.0,
            theme::with_alpha(color::on_surface(), 0.12),
        );
    }

    let s = 4.5;
    let stroke = egui::Stroke::new(1.4, fg);
    let c = rect.center();
    painter.line_segment(
        [egui::pos2(c.x - s, c.y - s), egui::pos2(c.x + s, c.y + s)],
        stroke,
    );
    painter.line_segment(
        [egui::pos2(c.x - s, c.y + s), egui::pos2(c.x + s, c.y - s)],
        stroke,
    );
}

/// A field caption in M3 "Label Small" style — small, muted, all-caps.
fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(color::on_surface_variant())
            .size(11.0)
            .strong(),
    );
}

/// Inline validation message shown directly under a form input.
/// Sized to match `field_label` (Label Small) so it doesn't visually
/// outweigh the field itself, and tinted with the M3 error color.
fn field_error(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).color(color::error()).size(11.0));
}

/// Two-button segmented control — one connected pill, no extra padding.
/// Returns `true` if the value changed.
fn segmented_two<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    current: &mut T,
    options: [(T, &str); 2],
) -> bool {
    let height = 32.0;
    let total_w = ui.available_width();
    let half = total_w * 0.5;
    let mut changed = false;

    let (rect, _) = ui.allocate_exact_size(egui::vec2(total_w, height), egui::Sense::hover());
    let painter = ui.painter().clone();
    let outline = egui::Stroke::new(1.0, color::outline());
    let rounding = egui::Rounding::same(height * 0.5);

    // Outer outlined pill.
    painter.rect(rect, rounding, color::surface(), outline);

    for (i, (val, label)) in options.iter().enumerate() {
        let seg_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + half * i as f32, rect.top()),
            egui::vec2(half, height),
        );
        let id = ui.id().with(("seg", i));
        let resp = ui.interact(seg_rect, id, egui::Sense::click());
        let selected = *current == *val;

        // Per-segment rounding: rounded only on the segment's outer side
        // so the two halves look continuous and any fill/state-layer
        // we paint follows the outer pill curve instead of being a
        // square that pokes past it.
        let seg_rounding = if i == 0 {
            egui::Rounding {
                nw: rounding.nw,
                sw: rounding.sw,
                ne: 0.0,
                se: 0.0,
            }
        } else {
            egui::Rounding {
                ne: rounding.ne,
                se: rounding.se,
                nw: 0.0,
                sw: 0.0,
            }
        };

        if selected {
            painter.rect_filled(seg_rect, seg_rounding, color::secondary_container());
        } else if resp.hovered() {
            painter.rect_filled(
                seg_rect,
                seg_rounding,
                theme::with_alpha(color::on_surface(), 0.06),
            );
        }

        // Divider between the two segments.
        if i == 0 {
            painter.line_segment(
                [
                    egui::pos2(seg_rect.right(), seg_rect.top() + 6.0),
                    egui::pos2(seg_rect.right(), seg_rect.bottom() - 6.0),
                ],
                outline,
            );
        }

        let fg = if selected {
            color::on_secondary_container()
        } else {
            color::on_surface()
        };
        let galley = ui.fonts(|f| {
            f.layout_no_wrap((*label).to_string(), egui::FontId::proportional(13.5), fg)
        });
        let pos = egui::pos2(
            seg_rect.center().x - galley.size().x * 0.5,
            seg_rect.center().y - galley.size().y * 0.5,
        );
        painter.galley(pos, galley, fg);

        if resp.clicked() && !selected {
            *current = *val;
            changed = true;
        }
    }
    // Re-stroke the outer outline so the divider doesn't bleed through it.
    painter.rect_stroke(rect, rounding, outline);
    changed
}

// ---------- Modals ----------

fn add_config_modal(ctx: &egui::Context, app: &mut App) {
    if !app.add_dialog.open {
        return;
    }
    let is_edit = app.add_dialog.is_edit();
    let title = if is_edit { "Edit config" } else { "Add config" };
    let busy = app.add_dialog.busy;
    let mut modal_state = app.add_dialog.modal_state.clone();

    let result = theme::modal_dialog_with_state(
        ctx,
        "add_config_modal",
        title,
        320.0,
        !busy,
        &mut modal_state,
        |ui, close| {
            ui.spacing_mut().item_spacing.y = 4.0;

            // ----- Name -----
            field_label(ui, "NAME");
            let name_resp = theme::input_singleline(ui, &mut app.add_dialog.name, "Config name");
            if name_resp.changed() {
                app.add_dialog.name_error = None;
            }
            if let Some(err) = app.add_dialog.name_error.clone() {
                field_error(ui, &err);
            }

            // ----- Source segmented selector -----
            ui.add_space(8.0);
            field_label(ui, "SOURCE");
            if segmented_two(
                ui,
                &mut app.add_dialog.kind,
                [
                    (SourceKind::Remote, "Remote URL"),
                    (SourceKind::Local, "Local file"),
                ],
            ) {
                // Switching source kind hides the now-irrelevant input,
                // so its stale error message would be confusing.
                app.add_dialog.path_error = None;
                app.add_dialog.url_error = None;
            }

            // ----- URL or File row -----
            ui.add_space(8.0);
            match app.add_dialog.kind {
                SourceKind::Remote => {
                    let url_resp = theme::input_singleline(ui, &mut app.add_dialog.url, "URL");
                    if url_resp.changed() {
                        app.add_dialog.url_error = None;
                    }
                    if let Some(err) = app.add_dialog.url_error.clone() {
                        field_error(ui, &err);
                    }
                }
                SourceKind::Local => {
                    ui.horizontal(|ui| {
                        let browse_w = 84.0;
                        let avail = ui.available_width() - browse_w - ui.spacing().item_spacing.x;
                        let path_resp = theme::input_singleline_sized(
                            ui,
                            &mut app.add_dialog.path,
                            "File path",
                            [avail.max(120.0), 32.0],
                        );
                        if path_resp.changed() {
                            app.add_dialog.path_error = None;
                        }
                        if ui
                            .add_sized([browse_w, 32.0], theme::tonal_button("Browse…"))
                            .clicked()
                        {
                            if let Some(p) = rfd::FileDialog::new()
                                .add_filter("JSON", &["json"])
                                .pick_file()
                            {
                                app.add_dialog.path = p.display().to_string();
                                app.add_dialog.path_error = None;
                            }
                        }
                    });
                    if let Some(err) = app.add_dialog.path_error.clone() {
                        field_error(ui, &err);
                    }
                }
            }

            // ----- Footer -----
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let primary_label = if busy {
                    if is_edit {
                        "Saving…"
                    } else {
                        "Adding…"
                    }
                } else if is_edit {
                    "Save"
                } else {
                    "Add"
                };
                if ui
                    .add_enabled(!busy, theme::filled_button(primary_label))
                    .clicked()
                {
                    app.submit_add_dialog();
                }
                if ui
                    .add_enabled(!busy, theme::text_button("Cancel"))
                    .clicked()
                {
                    *close = true;
                }
            });
        },
    );

    if result.close_requested {
        app.add_dialog.reset();
    } else {
        app.add_dialog.modal_state = modal_state;
    }
}

fn delete_confirm_modal(ctx: &egui::Context, app: &mut App) {
    let Some(slug) = app.delete_confirm.clone() else {
        return;
    };
    if !app.configs.iter().any(|c| c.slug == slug) {
        app.delete_confirm = None;
        return;
    }

    let result = theme::modal_dialog(
        ctx,
        "delete_confirm_modal",
        "Delete config",
        360.0,
        true,
        |ui, close| {
            ui.label(
                egui::RichText::new("This will delete the config and cannot be undone.")
                    .color(color::on_surface()),
            );
            ui.add_space(14.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme::destructive_filled_button("Delete")).clicked() {
                    let _ = app.bg_tx.send(BgCmd::DeleteConfig(slug.clone()));
                    *close = true;
                }
                ui.add_space(8.0);
                if ui.add(theme::text_button("Cancel")).clicked() {
                    *close = true;
                }
            });
        },
    );
    if result.close_requested {
        app.delete_confirm = None;
    }
}

fn destroy_confirm_modal(ctx: &egui::Context, app: &mut App) {
    if !app.destroy_confirm_open {
        return;
    }
    // When sing-box is running, the working directory can't be safely
    // purged (files are locked, and tearing them out from under a live
    // process is asking for trouble). Rather than showing the normal
    // confirm dialog with a disabled "Destroy" button — which reads as
    // a broken / missing button — surface a dedicated prompt that
    // explains the situation and asks the user to stop sing-box first.
    let running = app.proc.is_running();
    let (title, width) = if running {
        ("Stop sing-box first", 360.0)
    } else {
        ("Destroy working directory", 360.0)
    };
    let result = theme::modal_dialog(
        ctx,
        "destroy_confirm_modal",
        title,
        width,
        true,
        |ui, close| {
            if running {
                ui.label(
                    egui::RichText::new(
                        "Sing-box is still running. Please stop it before \
                     destroying the working directory.",
                    )
                    .color(color::on_surface()),
                );
                ui.add_space(14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(theme::filled_button("OK")).clicked() {
                        *close = true;
                    }
                });
            } else {
                ui.label(
                    egui::RichText::new(
                        "This will clear the current working directory. \
                     This action cannot be undone.",
                    )
                    .color(color::on_surface()),
                );
                ui.add_space(14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(theme::destructive_filled_button("Destroy"))
                        .clicked()
                    {
                        match shell::purge_directory(&app.paths.working_dir) {
                            Ok(()) => {
                                app.last_info = Some("Working directory cleared".into());
                                app.last_error = None;
                            }
                            Err(e) => app.last_error = Some(e.to_string()),
                        }
                        *close = true;
                    }
                    ui.add_space(8.0);
                    if ui.add(theme::text_button("Cancel")).clicked() {
                        *close = true;
                    }
                });
            }
        },
    );
    if result.close_requested {
        app.destroy_confirm_open = false;
    }
}

// ---------- About / license modal ----------
//
// Embeds the canonical GPLv3 text at compile time (see the `LICENSE`
// file at the workspace root) so the dialog is fully self-contained
// — no extra files required next to the .exe at runtime.
//
// The dialog also carries an explicit "runtime components disclaimer"
// that makes two facts unambiguous to any reader:
//   1. This launcher does NOT bundle, redistribute, or otherwise
//      provide the `sing-box` core or any other runtime-required
//      component. Users acquire those on their own.
//   2. `sing-box` is a separate upstream open-source project authored
//      by third parties; its license, copyright, and any legal risk
//      arising from its use belong to its respective authors and end
//      users. The launcher's author makes no warranty about the
//      legality of using sing-box in any jurisdiction (notably,
//      mainland China carries known regulatory risk for proxy tools).
//
// Surfacing both facts inside the app, behind a button labelled
// "View license & disclaimer", is the conservative reading of "best
// practice" the user asked for: a) the GPL §17 short notice + full
// text are visible from the running program (satisfying the GPL's
// "interactive" notice expectations), and b) the upstream-attribution
// + disclaimer is shown together so a reader cannot miss it.
const LICENSE_TEXT: &str = include_str!("../LICENSE");

fn about_modal(ctx: &egui::Context, app: &mut App) {
    if !app.about_open {
        return;
    }

    // Size the About dialog from the live viewport every frame. The
    // dialog follows the outer window; if the window becomes too short
    // for the text, only the body scrolls while the Close button remains
    // pinned at the bottom. These are exact layout-budget values rather
    // than a guessed "screen minus N" so the outer frame never outgrows
    // the current viewport at the minimum window size.
    let screen = ctx.screen_rect();
    let compact = screen.width() < 580.0 || screen.height() < 620.0;
    let edge_gap = if compact { 8.0 } else { 16.0 };
    let frame_pad = if compact { 8.0 } else { 16.0 };
    let modal_w = (screen.width() - edge_gap * 2.0 - frame_pad * 2.0).max(0.0);
    let chrome_h = 28.0  // title row, including the close icon hit target
        + theme::modal::HEADER_GAP
        + 14.0           // body/footer gap below the scroll area
        + 28.0           // Close button row
        + frame_pad * 2.0;
    let body_max_h = (screen.height() - edge_gap * 2.0 - chrome_h).max(48.0);

    let mut modal_state = app.about_modal_state.clone();
    let result = theme::modal_dialog_sized_with_state(
        ctx,
        "about_modal",
        "About",
        modal_w,
        body_max_h,
        frame_pad,
        true,
        &mut modal_state,
        |ui, body_max_h, close| {
            // ----- Scrollable body -----
            // All variable-height content (header, disclaimer, GPL
            // short notice, full license box) lives inside this outer
            // scroll. The Close button is rendered AFTER the scroll
            // area so it stays pinned at the modal's bottom and is
            // always reachable regardless of how the user scrolls.
            egui::ScrollArea::vertical()
                .id_source("about_body_scroll")
                .auto_shrink([false, true])
                .max_height(body_max_h)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} v{}",
                            crate::APP_TITLE,
                            env!("CARGO_PKG_VERSION")
                        ))
                        .color(color::on_surface())
                        .strong()
                        .size(15.0),
                    );
                    ui.label(
                        egui::RichText::new("Copyright © 2026 ashton2914")
                            .color(color::on_surface_variant()),
                    );

                    ui.add_space(10.0);

                    // -- Runtime-components / upstream disclaimer --
                    field_label(ui, "RUNTIME COMPONENTS & UPSTREAM DISCLAIMER");
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "This program is a launcher / manager for sing-box on Windows. \
                             It does NOT bundle, redistribute, or provide the sing-box core \
                             binary or any other runtime-required component — users must \
                             obtain those components themselves.\n\n\
                             sing-box is an independent third-party open-source project; its \
                             source, license, and copyright belong to its respective authors. \
                             The use of sing-box may carry legal risk in certain jurisdictions \
                             (including, but not limited to, mainland China). The author of \
                             this launcher makes no warranty regarding the legality of using \
                             sing-box in any jurisdiction; users are solely responsible for \
                             complying with all applicable laws and regulations.",
                        )
                        .color(color::on_surface()),
                    );

                    ui.add_space(14.0);

                    // -- GPLv3 short notice -----------------------
                    field_label(ui, "LICENSE — GNU GENERAL PUBLIC LICENSE v3 (or later)");
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "This program is free software: you can redistribute it and/or \
                             modify it under the terms of the GNU General Public License as \
                             published by the Free Software Foundation, either version 3 of \
                             the License, or (at your option) any later version.\n\n\
                             This program is distributed in the hope that it will be useful, \
                             but WITHOUT ANY WARRANTY; without even the implied warranty of \
                             MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the \
                             GNU General Public License for more details.",
                        )
                        .color(color::on_surface()),
                    );

                    ui.add_space(8.0);

                    // -- Full GPLv3 text in a compact monospace box.
                    // Its own ScrollArea so the user can browse the
                    // 35KB license without dragging the outer scroll
                    // hundreds of pixels. The inner area's height is
                    // intentionally short to keep the overall modal
                    // tidy.
                    egui::Frame::none()
                        .fill(color::surface_container_lowest())
                        .rounding(egui::Rounding::same(radius::MD))
                        .inner_margin(egui::Margin::same(10.0))
                        .show(ui, |ui| {
                            let inner_width = (ui.available_width()).max(0.0);
                            ui.set_min_width(inner_width);
                            ui.set_max_width(inner_width);
                            egui::ScrollArea::vertical()
                                .auto_shrink([false; 2])
                                .drag_to_scroll(false)
                                .max_height(180.0)
                                .id_source("about_license_scroll")
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(LICENSE_TEXT)
                                                .color(color::on_surface_variant())
                                                .monospace()
                                                .size(11.5),
                                        )
                                        .wrap(),
                                    );
                                });
                        });
                });

            ui.add_space(14.0);

            // Close button is OUTSIDE the scroll area on purpose: the
            // user must always have a one-click dismiss path even on
            // tiny windows where the body is heavily scrolled.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme::tonal_button("Close")).clicked() {
                    *close = true;
                }
            });
        },
    );
    if result.close_requested {
        app.about_open = false;
        app.about_modal_state.reset();
    } else {
        app.about_modal_state = modal_state;
    }
}

/// Modal that surfaces the full multi-line `<core> version` output —
/// environment, build tags, revision — captured by
/// `App::ensure_core_version`. The status toolbar shows just the
/// short version string; this modal is the "click to see everything"
/// affordance.
fn core_version_modal(ctx: &egui::Context, app: &mut App) {
    if !app.core_version_open {
        return;
    }

    let full = app.core_version.full.clone().unwrap_or_else(|| {
        "(no version info available — the core lookup has not completed yet)".to_owned()
    });
    let core_name = app
        .settings
        .selected_core
        .clone()
        .unwrap_or_else(|| "(no core selected)".to_owned());

    // Size the dialog from the viewport every frame, mirroring
    // `about_modal`. The body scrolls; the Close button stays pinned.
    let screen = ctx.screen_rect();
    let compact = screen.width() < 580.0 || screen.height() < 620.0;
    let edge_gap = if compact { 8.0 } else { 16.0 };
    let frame_pad = if compact { 8.0 } else { 16.0 };
    let modal_w = (screen.width() - edge_gap * 2.0 - frame_pad * 2.0).max(0.0);
    let chrome_h = 28.0  // title row
        + theme::modal::HEADER_GAP
        + 14.0           // body/footer gap
        + 28.0           // Close button row
        + frame_pad * 2.0;
    let body_max_h = (screen.height() - edge_gap * 2.0 - chrome_h).max(48.0);

    let mut modal_state = app.core_version_modal_state.clone();
    let result = theme::modal_dialog_sized_with_state(
        ctx,
        "core_version_modal",
        "Core Version",
        modal_w,
        body_max_h,
        frame_pad,
        true,
        &mut modal_state,
        |ui, body_max_h, close| {
            egui::ScrollArea::vertical()
                .id_source("core_version_body_scroll")
                .auto_shrink([false, true])
                .max_height(body_max_h)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(core_name.as_str())
                            .color(color::on_surface())
                            .strong()
                            .size(14.0),
                    );
                    ui.add_space(8.0);

                    // Monospace block so columns in tag lists line up
                    // exactly the way `sing-box version` prints them.
                    egui::Frame::none()
                        .fill(color::surface_container_lowest())
                        .rounding(egui::Rounding::same(radius::MD))
                        .inner_margin(egui::Margin::same(10.0))
                        .show(ui, |ui| {
                            let inner_width = ui.available_width().max(0.0);
                            ui.set_min_width(inner_width);
                            ui.set_max_width(inner_width);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(full.as_str())
                                        .color(color::on_surface_variant())
                                        .monospace()
                                        .size(12.0),
                                )
                                .wrap(),
                            );
                        });
                });

            ui.add_space(14.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme::tonal_button("Close")).clicked() {
                    *close = true;
                }
            });
        },
    );
    if result.close_requested {
        app.core_version_open = false;
        app.core_version_modal_state.reset();
    } else {
        app.core_version_modal_state = modal_state;
    }
}
