use eframe::egui;

use crate::app::{App, BgCmd, SourceKind};
use crate::config::entry::Source;
use crate::core::shell;
use crate::theme::{self, color, radius};

pub fn show(ui: &mut egui::Ui, app: &mut App) {
    // Outer frame has zero horizontal margin so the vertical scrollbar
    // hugs the window's right edge. Per-side padding is reapplied to the
    // content *inside* the ScrollArea via an inner Frame.
    egui::Frame::none()
        .inner_margin(egui::Margin {
            left: 0.0,
            right: 0.0,
            top: 6.0,
            bottom: 0.0,
        })
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    egui::Frame::none()
                        .inner_margin(egui::Margin {
                            left: 16.0,
                            right: 16.0,
                            top: 0.0,
                            bottom: 12.0,
                        })
                        .show(ui, |ui| {
                            run_card(ui, app);
                            ui.add_space(8.0);
                            config_card(ui, app);
                            ui.add_space(8.0);
                            settings_card(ui, app);
                        });
                });
        });

    add_config_modal(ui.ctx(), app);
    delete_confirm_modal(ui.ctx(), app);
    destroy_confirm_modal(ui.ctx(), app);
}

// ---------- Config card ----------

fn config_card(ui: &mut egui::Ui, app: &mut App) {
    theme::card(ui, |ui| {
        theme::section_title(ui, "Configs");
        ui.add_space(6.0);

        // ----- selector + Add button on one row -----
        // The Add button is placed first in a right-to-left layout so it
        // anchors to the right edge; `config_combo` then fills the
        // remaining space to its left.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme::filled_button("+ Add")).clicked() {
                    app.add_dialog.reset();
                    app.add_dialog.open = true;
                }
                let remaining = ui.available_width();
                config_combo(ui, app, remaining);
            });
        });

        // ----- detail panel -----
        ui.add_space(8.0);
        match app.selected_entry().cloned() {
            None => {
                ui.label(
                    egui::RichText::new("Select a config above, or click + Add to create one.")
                        .color(color::ON_SURFACE_VARIANT),
                );
            }
            Some(entry) => {
                let kind_label = entry.metadata.source.kind_label();
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
                        detail_label(ui, "TYPE");
                        detail_value(ui, kind_label);
                        ui.end_row();

                        detail_label(ui, "LAST UPDATED");
                        detail_value(ui, &last);
                        ui.end_row();
                    });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let update_label = if is_remote { "Update" } else { "Re-import" };
                    if ui.add(theme::filled_button(update_label)).clicked() {
                        let _ = app.bg_tx.send(BgCmd::UpdateConfig(entry.slug.clone()));
                    }
                    if ui.add(theme::tonal_button("Edit")).clicked() {
                        app.add_dialog.open_for_edit(&entry);
                    }
                    if ui.add(theme::tonal_button("Open Folder")).clicked() {
                        if let Err(e) = shell::open(&entry.folder) {
                            app.last_error = Some(e.to_string());
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

/// Custom ComboBox replacement that paints a real rounded-pill chip next
/// to each config name. egui's `ComboBox` only accepts a single
/// `WidgetText`, so we hand-roll the closed button + popup to get a chip
/// Custom ComboBox replacement that paints a real rounded-pill chip next
/// to each config name. egui's `ComboBox` only accepts a single
/// `WidgetText` (which can't carry a rounded pill paint), and its
/// `popup_below_widget` forces a frame style we can't theme. So we
/// hand-roll both the closed combo and the popup to make them feel like a
/// single unified widget:
///   * shared surface tone, shared outline color
///   * when open, the closed combo's bottom corners flatten and the popup
///     glues to it with matching top-flat corners — together they form
///     one outlined shape
///   * selected row uses a subtle 12% PRIMARY tonal layer instead of a
///     full primary-container fill, so the chip and name keep their
///     normal colors and the dropdown reads as continuous
fn config_combo(ui: &mut egui::Ui, app: &mut App, width: f32) {
    let popup_id = ui.make_persistent_id("config_combo_v3");
    let is_open = ui.memory(|m| m.is_popup_open(popup_id));

    let height = 36.0;
    let response = ui.allocate_response(egui::vec2(width, height), egui::Sense::click());
    let rect = response.rect;
    let painter = ui.painter().clone();

    let outline = egui::Stroke::new(1.0, color::OUTLINE_VARIANT);
    let surface = color::SURFACE_CONTAINER;

    // Closed-combo rounding: bottom corners go flat when popup is open.
    let combo_rounding = if is_open {
        egui::Rounding {
            nw: radius::SM,
            ne: radius::SM,
            sw: 0.0,
            se: 0.0,
        }
    } else {
        egui::Rounding::same(radius::SM)
    };
    painter.rect(rect, combo_rounding, surface, outline);
    if response.hovered() && !is_open {
        painter.rect_filled(
            rect,
            combo_rounding,
            theme::with_alpha(color::ON_SURFACE, 0.04),
        );
    }

    let inner = rect.shrink2(egui::vec2(12.0, 0.0));
    let center_y = inner.center().y;

    // Chevron (down-triangle) on the right.
    let chev_color = color::ON_SURFACE_VARIANT;
    let chev_w = 9.0;
    let chev_h = 5.0;
    let chev_cx = inner.right() - chev_w * 0.5;
    let chev_cy = center_y;
    let chev = vec![
        egui::pos2(chev_cx - chev_w * 0.5, chev_cy - chev_h * 0.5),
        egui::pos2(chev_cx + chev_w * 0.5, chev_cy - chev_h * 0.5),
        egui::pos2(chev_cx, chev_cy + chev_h * 0.5),
    ];
    painter.add(egui::Shape::convex_polygon(
        chev,
        chev_color,
        egui::Stroke::NONE,
    ));
    let content_right = inner.right() - chev_w - 8.0;

    // Left side: name + chip OR placeholder.
    let name_font = egui::FontId::proportional(14.0);
    if let Some(entry) = app.selected_entry() {
        let kind = entry.metadata.source.kind_label();
        let name_galley = ui.fonts(|f| {
            f.layout_no_wrap(entry.metadata.name.clone(), name_font, color::ON_SURFACE)
        });
        let ns = name_galley.size();
        painter.galley(
            egui::pos2(inner.left(), center_y - ns.y * 0.5),
            name_galley,
            color::ON_SURFACE,
        );
        let cs = theme::chip_size(ui, kind);
        let chip_left = inner.left() + ns.x + 8.0;
        if chip_left + cs.x <= content_right {
            let chip_center = egui::pos2(chip_left + cs.x * 0.5, center_y);
            let _ = theme::paint_chip(ui, &painter, chip_center, kind);
        }
    } else {
        let placeholder = ui.fonts(|f| {
            f.layout_no_wrap("(none)".into(), name_font, color::ON_SURFACE_VARIANT)
        });
        let ps = placeholder.size();
        painter.galley(
            egui::pos2(inner.left(), center_y - ps.y * 0.5),
            placeholder,
            color::ON_SURFACE_VARIANT,
        );
    }

    if response.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }

    // Snapshot config data so we don't borrow `app` across the popup closure.
    let configs_snapshot: Vec<(String, String, String)> = app
        .configs
        .iter()
        .map(|c| {
            (
                c.slug.clone(),
                c.metadata.name.clone(),
                c.metadata.source.kind_label().to_string(),
            )
        })
        .collect();
    let current_selected = app.settings.selected_config.clone();
    let mut new_selected: Option<String> = None;
    let mut close_requested = false;

    if is_open {
        let area = egui::Area::new(popup_id.with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.left_bottom())
            .show(ui.ctx(), |ui| {
                // Frame glued to the closed combo: top corners flat, same
                // surface + outline so the two pieces read as one shape.
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

                        if configs_snapshot.is_empty() {
                            ui.label(
                                egui::RichText::new("(no configs — click + Add)")
                                    .color(color::ON_SURFACE_VARIANT)
                                    .italics(),
                            );
                            return;
                        }

                        for (slug, name, kind) in &configs_snapshot {
                            let selected =
                                current_selected.as_deref() == Some(slug.as_str());
                            let item_resp = ui.allocate_response(
                                egui::vec2(ui.available_width(), 32.0),
                                egui::Sense::click(),
                            );
                            let r = item_resp.rect;
                            let p = ui.painter().clone();

                            // Subtle tonal selection layer; no full primary
                            // container fill, so the chip + name keep their
                            // normal colors and the row reads as part of the
                            // combo surface.
                            let item_bg = if selected {
                                theme::with_alpha(color::PRIMARY, 0.14)
                            } else if item_resp.hovered() {
                                theme::with_alpha(color::ON_SURFACE, 0.06)
                            } else {
                                egui::Color32::TRANSPARENT
                            };
                            p.rect_filled(r, egui::Rounding::same(radius::XS), item_bg);

                            let inner = r.shrink2(egui::vec2(8.0, 0.0));
                            let cy = inner.center().y;
                            let name_galley = ui.fonts(|f| {
                                f.layout_no_wrap(
                                    name.clone(),
                                    egui::FontId::proportional(14.0),
                                    color::ON_SURFACE,
                                )
                            });
                            let ns = name_galley.size();
                            p.galley(
                                egui::pos2(inner.left(), cy - ns.y * 0.5),
                                name_galley,
                                color::ON_SURFACE,
                            );
                            let cs = theme::chip_size(ui, kind);
                            let chip_center =
                                egui::pos2(inner.left() + ns.x + 8.0 + cs.x * 0.5, cy);
                            let _ = theme::paint_chip(ui, &p, chip_center, kind);

                            if item_resp.clicked() {
                                new_selected = Some(slug.clone());
                                close_requested = true;
                            }
                        }
                    });
            });

        // Close on outside click. The Area's response covers the whole
        // popup; the closed combo handles its own clicks via toggle.
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

    if let Some(sel) = new_selected {
        if app.settings.selected_config.as_deref() != Some(sel.as_str()) {
            app.settings.selected_config = Some(sel);
            app.persist_settings();
        }
    }
    if close_requested {
        ui.memory_mut(|m| m.close_popup());
    }
}

fn detail_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(color::ON_SURFACE_VARIANT)
            .size(11.0)
            .strong(),
    );
}

fn detail_value(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(color::ON_SURFACE));
}

// ---------- Settings card ----------

fn settings_card(ui: &mut egui::Ui, app: &mut App) {
    theme::card(ui, |ui| {
        theme::section_title(ui, "Settings");
        ui.add_space(8.0);

        theme::subsection_title(ui, "APP");
        ui.add_space(2.0);
        ui.indent("app_settings", |ui| {
            let mut s_changed = false;
            if ui
                .checkbox(&mut app.settings.auto_start, "Auto start up")
                .changed()
            {
                s_changed = true;
            }
            if ui
                .checkbox(&mut app.settings.auto_update, "Config auto update")
                .changed()
            {
                s_changed = true;
            }
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Auto update interval (min)")
                        .color(color::ON_SURFACE_VARIANT),
                );
                if ui
                    .add(
                        egui::DragValue::new(&mut app.settings.update_interval_minutes)
                            .range(1..=10080),
                    )
                    .changed()
                {
                    s_changed = true;
                }
            });
            if s_changed {
                app.persist_settings();
            }
        });

        ui.add_space(10.0);
        theme::subsection_title(ui, "SING-BOX");
        ui.add_space(2.0);
        ui.indent("singbox_settings", |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Core").color(color::ON_SURFACE_VARIANT));
                let current = app
                    .settings
                    .selected_core
                    .clone()
                    .unwrap_or_else(|| "(none)".to_string());
                let mut new_sel = app.settings.selected_core.clone();
                egui::ComboBox::from_id_source("core_combo")
                    .selected_text(&current)
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        if app.cores.is_empty() {
                            ui.label(
                                egui::RichText::new("(no *.exe under core/)")
                                    .color(color::ON_SURFACE_VARIANT)
                                    .italics(),
                            );
                        }
                        for name in &app.cores {
                            ui.selectable_value(&mut new_sel, Some(name.clone()), name);
                        }
                    });
                if new_sel != app.settings.selected_core {
                    app.settings.selected_core = new_sel;
                    app.persist_settings();
                }
                if ui
                    .add(theme::icon_button("⟳"))
                    .on_hover_text("Refresh core list")
                    .clicked()
                {
                    app.refresh_listings();
                }
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
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
    });
}

// ---------- Run / status / logs ----------

fn run_card(ui: &mut egui::Ui, app: &mut App) {
    let running = app.proc.is_running();

    theme::card(ui, |ui| {
        ui.horizontal(|ui| {
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
            ui.vertical(|ui| {
                let (status_color, status_text) = if running {
                    (color::SUCCESS, "Running")
                } else {
                    (color::ON_SURFACE_VARIANT, "Stopped")
                };
                ui.label(
                    egui::RichText::new("STATUS")
                        .color(color::ON_SURFACE_VARIANT)
                        .size(11.0)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(status_text)
                        .color(status_color)
                        .size(20.0)
                        .strong(),
                );
            });
            // Show/Hide log toggle, right-aligned on the same row as the
            // status block — saves a whole row of vertical space.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if app.log_visible { "Hide log" } else { "Show log" };
                if ui.add(theme::text_button(label)).clicked() {
                    app.log_visible = !app.log_visible;
                }
            });
        });

        if let Some(err) = app.last_error.clone() {
            ui.add_space(8.0);
            banner(ui, &err, color::ERROR_CONTAINER, color::ERROR);
        }

        if app.log_visible {
            ui.add_space(10.0);
            egui::Frame::none()
                .fill(color::SURFACE_CONTAINER_LOWEST)
                .rounding(egui::Rounding::same(radius::MD))
                .inner_margin(egui::Margin::same(10.0))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .max_height(200.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if app.logs.is_empty() {
                                ui.label(
                                    egui::RichText::new("(no logs yet)")
                                        .color(color::ON_SURFACE_VARIANT)
                                        .italics(),
                                );
                            }
                            for log in &app.logs {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(&log.timestamp)
                                            .color(color::ON_SURFACE_VARIANT)
                                            .monospace(),
                                    );
                                    ui.label(
                                        egui::RichText::new(&log.message)
                                            .color(color::ON_SURFACE)
                                            .monospace(),
                                    );
                                });
                            }
                        });
                });
        }
    });
}

// ---------- Helpers ----------

fn banner(ui: &mut egui::Ui, text: &str, bg: egui::Color32, fg: egui::Color32) {
    egui::Frame::none()
        .fill(bg)
        .rounding(egui::Rounding::same(radius::MD))
        .inner_margin(egui::Margin {
            left: 12.0,
            right: 12.0,
            top: 8.0,
            bottom: 8.0,
        })
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(fg));
        });
}

fn modal_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(color::SURFACE_CONTAINER_HIGH)
        .rounding(egui::Rounding::same(radius::XL))
        .inner_margin(egui::Margin::same(16.0))
        .stroke(egui::Stroke::new(1.0, color::OUTLINE_VARIANT))
        .shadow(egui::epaint::Shadow {
            offset: egui::vec2(0.0, 8.0),
            blur: 24.0,
            spread: 0.0,
            color: egui::Color32::from_black_alpha(120),
        })
}

/// A field caption in M3 "Label Small" style — small, muted, all-caps.
fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(color::ON_SURFACE_VARIANT)
            .size(11.0)
            .strong(),
    );
}

/// A single-line text input with the modal's compact look.
fn modal_text_edit<'t>(
    text: &'t mut String,
    hint: &str,
) -> egui::TextEdit<'t> {
    egui::TextEdit::singleline(text)
        .hint_text(hint)
        .desired_width(f32::INFINITY)
        .margin(egui::vec2(10.0, 6.0))
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
    let outline = egui::Stroke::new(1.0, color::OUTLINE);
    let rounding = egui::Rounding::same(height * 0.5);

    // Outer outlined pill.
    painter.rect(rect, rounding, color::SURFACE, outline);

    for (i, (val, label)) in options.iter().enumerate() {
        let seg_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + half * i as f32, rect.top()),
            egui::vec2(half, height),
        );
        let id = ui.id().with(("seg", i));
        let resp = ui.interact(seg_rect, id, egui::Sense::click());
        let selected = *current == *val;

        // Selected segment: filled tonal layer with rounded corners only on
        // the segment's outer side so the two halves look continuous.
        if selected {
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
            painter.rect_filled(seg_rect, seg_rounding, color::SECONDARY_CONTAINER);
        } else if resp.hovered() {
            painter.rect_filled(
                seg_rect,
                egui::Rounding::ZERO,
                theme::with_alpha(color::ON_SURFACE, 0.06),
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
            color::ON_SECONDARY_CONTAINER
        } else {
            color::ON_SURFACE
        };
        let galley = ui.fonts(|f| {
            f.layout_no_wrap(
                (*label).to_string(),
                egui::FontId::proportional(13.5),
                fg,
            )
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
    let mut still_open = true;
    egui::Window::new(
        egui::RichText::new(title)
            .color(color::ON_SURFACE)
            .size(16.0)
            .strong(),
    )
    .collapsible(false)
    .resizable(false)
    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
    .frame(modal_frame())
    .open(&mut still_open)
    .show(ctx, |ui| {
        ui.set_min_width(320.0);
        ui.set_max_width(320.0);
        ui.spacing_mut().item_spacing.y = 4.0;

        // ----- Name -----
        field_label(ui, "NAME");
        ui.add(modal_text_edit(&mut app.add_dialog.name, "My Subscription"));

        // ----- Source segmented selector -----
        ui.add_space(8.0);
        field_label(ui, "SOURCE");
        segmented_two(
            ui,
            &mut app.add_dialog.kind,
            [
                (SourceKind::Remote, "Remote URL"),
                (SourceKind::Local, "Local file"),
            ],
        );

        // ----- URL or File row (no separate caption — the segmented
        // selector already conveys what kind of value goes here). -----
        ui.add_space(8.0);
        match app.add_dialog.kind {
            SourceKind::Remote => {
                ui.add(modal_text_edit(
                    &mut app.add_dialog.url,
                    "https://example.com/sub.json",
                ));
            }
            SourceKind::Local => {
                ui.horizontal(|ui| {
                    let browse_w = 84.0;
                    let avail = ui.available_width() - browse_w - ui.spacing().item_spacing.x;
                    ui.add_sized(
                        [avail.max(120.0), 32.0],
                        egui::TextEdit::singleline(&mut app.add_dialog.path)
                            .hint_text(r"C:\path\to\config.json")
                            .margin(egui::vec2(10.0, 6.0)),
                    );
                    if ui
                        .add_sized([browse_w, 32.0], theme::tonal_button("Browse…"))
                        .clicked()
                    {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            app.add_dialog.path = p.display().to_string();
                        }
                    }
                });
            }
        }

        // ----- Footer -----
        ui.add_space(14.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let busy = app.add_dialog.busy;
            let primary_label = if busy {
                if is_edit { "Saving…" } else { "Adding…" }
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
                app.add_dialog.reset();
            }
        });
    });
    if !still_open && !app.add_dialog.busy {
        app.add_dialog.reset();
    }
}

fn delete_confirm_modal(ctx: &egui::Context, app: &mut App) {
    let Some(slug) = app.delete_confirm.clone() else {
        return;
    };
    let entry = app.configs.iter().find(|c| c.slug == slug).cloned();
    let Some(entry) = entry else {
        app.delete_confirm = None;
        return;
    };

    let mut still_open = true;
    egui::Window::new(
        egui::RichText::new("Delete config")
            .color(color::ON_SURFACE)
            .size(18.0)
            .strong(),
    )
    .collapsible(false)
    .resizable(false)
    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
    .frame(modal_frame())
    .open(&mut still_open)
    .show(ctx, |ui| {
        ui.set_min_width(360.0);
        ui.label(
            egui::RichText::new(format!(
                "Delete '{}' and wipe its folder?\n{}",
                entry.metadata.name,
                entry.folder.display()
            ))
            .color(color::ON_SURFACE),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("This cannot be undone.")
                .color(color::ON_SURFACE_VARIANT),
        );
        ui.add_space(14.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(theme::destructive_filled_button("Delete")).clicked() {
                let _ = app.bg_tx.send(BgCmd::DeleteConfig(slug.clone()));
                app.delete_confirm = None;
            }
            ui.add_space(8.0);
            if ui.add(theme::text_button("Cancel")).clicked() {
                app.delete_confirm = None;
            }
        });
    });
    if !still_open {
        app.delete_confirm = None;
    }
}

fn destroy_confirm_modal(ctx: &egui::Context, app: &mut App) {
    if !app.destroy_confirm_open {
        return;
    }
    let mut still_open = true;
    egui::Window::new(
        egui::RichText::new("Destroy working directory")
            .color(color::ON_SURFACE)
            .size(18.0)
            .strong(),
    )
    .collapsible(false)
    .resizable(false)
    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
    .frame(modal_frame())
    .open(&mut still_open)
    .show(ctx, |ui| {
        ui.set_min_width(360.0);
        ui.label(
            egui::RichText::new(format!(
                "Will wipe everything in:\n{}",
                app.paths.working_dir.display()
            ))
            .color(color::ON_SURFACE),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("This cannot be undone.")
                .color(color::ON_SURFACE_VARIANT),
        );
        ui.add_space(14.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let running = app.proc.is_running();
            let btn = ui
                .add_enabled(!running, theme::destructive_filled_button("Destroy"))
                .on_disabled_hover_text("Stop sing-box first");
            if btn.clicked() {
                match shell::purge_directory(&app.paths.working_dir) {
                    Ok(()) => {
                        app.last_info = Some("Working directory cleared".into());
                        app.last_error = None;
                    }
                    Err(e) => app.last_error = Some(e.to_string()),
                }
                app.destroy_confirm_open = false;
            }
            ui.add_space(8.0);
            if ui.add(theme::text_button("Cancel")).clicked() {
                app.destroy_confirm_open = false;
            }
        });
    });
    if !still_open {
        app.destroy_confirm_open = false;
    }
}
