use eframe::egui;

use crate::app::{App, BgCmd, SourceKind};
use crate::config::entry::Source;
use crate::core::shell;
use crate::theme::{self, color, radius};

pub fn show(ui: &mut egui::Ui, app: &mut App) {
    // The page cards must always be the window width minus fixed insets.
    // Do not derive this from any nested `available_width`: ScrollArea can
    // change that value when its overflow state changes, which is exactly
    // what made the cards jump when toggling the log panel.
    const PAGE_LEFT: f32 = 20.0;
    const PAGE_RIGHT: f32 = 40.0; // includes the thin scrollbar gutter
    let card_width = (ui.ctx().screen_rect().width() - PAGE_LEFT - PAGE_RIGHT).max(240.0);

    egui::Frame::none()
        .inner_margin(egui::Margin {
            left: 0.0,
            right: 0.0,
            top: 6.0,
            bottom: 0.0,
        })
        .show(ui, |ui| {
            ui.spacing_mut().scroll.floating = true;

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    egui::Frame::none()
                        .inner_margin(egui::Margin {
                            left: PAGE_LEFT,
                            right: PAGE_RIGHT,
                            top: 0.0,
                            bottom: 12.0,
                        })
                        .show(ui, |ui| {
                            run_card(ui, app, card_width);
                            ui.add_space(8.0);
                            config_card(ui, app, card_width);
                            ui.add_space(8.0);
                            settings_card(ui, app, card_width);
                        });
                });
        });

    add_config_modal(ui.ctx(), app);
    delete_confirm_modal(ui.ctx(), app);
    destroy_confirm_modal(ui.ctx(), app);
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
                        .color(color::ON_SURFACE_VARIANT),
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
        let name_galley = ui
            .fonts(|f| f.layout_no_wrap(entry.metadata.name.clone(), name_font, color::ON_SURFACE));
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
        let placeholder =
            ui.fonts(|f| f.layout_no_wrap("(none)".into(), name_font, color::ON_SURFACE_VARIANT));
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
                                    .color(color::ON_SURFACE_VARIANT),
                            );
                            return;
                        }

                        for (slug, name, kind) in &configs_snapshot {
                            let selected = current_selected.as_deref() == Some(slug.as_str());
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

/// Hand-painted core selector — same combo language as `config_combo` so
/// the Settings card and the Configs card read as one design system.
fn core_combo(ui: &mut egui::Ui, app: &mut App, width: f32) {
    let popup_id = ui.make_persistent_id("core_combo_v1");
    let is_open = ui.memory(|m| m.is_popup_open(popup_id));

    let height = 32.0;
    let response = ui.allocate_response(egui::vec2(width, height), egui::Sense::click());
    let rect = response.rect;
    let painter = ui.painter().clone();

    let outline = egui::Stroke::new(1.0, color::OUTLINE_VARIANT);
    let surface = color::SURFACE_CONTAINER;
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

    // Chevron.
    let chev_color = color::ON_SURFACE_VARIANT;
    let chev_w = 9.0;
    let chev_h = 5.0;
    let chev_cx = inner.right() - chev_w * 0.5;
    let chev = vec![
        egui::pos2(chev_cx - chev_w * 0.5, center_y - chev_h * 0.5),
        egui::pos2(chev_cx + chev_w * 0.5, center_y - chev_h * 0.5),
        egui::pos2(chev_cx, center_y + chev_h * 0.5),
    ];
    painter.add(egui::Shape::convex_polygon(
        chev,
        chev_color,
        egui::Stroke::NONE,
    ));

    let name_font = egui::FontId::proportional(13.5);
    let (text, color_) = match app.settings.selected_core.as_deref() {
        Some(name) => (name.to_string(), color::ON_SURFACE),
        None => ("(none)".to_string(), color::ON_SURFACE_VARIANT),
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(text, name_font, color_));
    painter.galley(
        egui::pos2(inner.left(), center_y - galley.size().y * 0.5),
        galley,
        color_,
    );

    if response.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }

    let cores_snapshot: Vec<String> = app.cores.clone();
    let current = app.settings.selected_core.clone();
    let mut new_selected: Option<Option<String>> = None;
    let mut close_requested = false;

    if is_open {
        let area = egui::Area::new(popup_id.with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.left_bottom())
            .show(ui.ctx(), |ui| {
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

                        if cores_snapshot.is_empty() {
                            ui.label(
                                egui::RichText::new("(no *.exe under core/)")
                                    .color(color::ON_SURFACE_VARIANT),
                            );
                            return;
                        }

                        for name in &cores_snapshot {
                            let selected = current.as_deref() == Some(name.as_str());
                            let item_resp = ui.allocate_response(
                                egui::vec2(ui.available_width(), 28.0),
                                egui::Sense::click(),
                            );
                            let r = item_resp.rect;
                            let p = ui.painter().clone();
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
                            let g = ui.fonts(|f| {
                                f.layout_no_wrap(
                                    name.clone(),
                                    egui::FontId::proportional(13.5),
                                    color::ON_SURFACE,
                                )
                            });
                            p.galley(
                                egui::pos2(inner.left(), cy - g.size().y * 0.5),
                                g,
                                color::ON_SURFACE,
                            );

                            if item_resp.clicked() {
                                new_selected = Some(Some(name.clone()));
                                close_requested = true;
                            }
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

    if let Some(sel) = new_selected {
        if app.settings.selected_core != sel {
            app.settings.selected_core = sel;
            app.persist_settings();
        }
    }
    if close_requested {
        ui.memory_mut(|m| m.close_popup());
    }
}

fn detail_value(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(color::ON_SURFACE));
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
            // Admin-mode toggles come first because they're the most
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
            ui.horizontal(|ui| {
                ui.label(theme::setting_label("Auto update interval (hours)"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [56.0, 22.0],
                            egui::DragValue::new(&mut app.settings.update_interval_hours)
                                .range(1..=720),
                        )
                        .changed()
                    {
                        s_changed = true;
                    }
                });
            });
            if s_changed {
                app.persist_settings();
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

fn subtle_divider(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        egui::Stroke::new(1.0, theme::with_alpha(color::OUTLINE_VARIANT, 0.45)),
    );
}

// ---------- Run / status / logs ----------

/// Small pill chip showing the current process integrity. Painted
/// inline (no extra row of vertical space) and color-coded so a glance
/// at the top card tells the user whether TUN mode will work:
///   * Administrator → success-tinted pill
///   * Standard user → tertiary-tinted pill (the same neutral hue used
///                     for the not-yet-elevated shield in the Settings card)
fn elevation_pill(ui: &mut egui::Ui) {
    use crate::core::elevation;

    let elevated = elevation::is_elevated();
    let (label, fg, bg) = if elevated {
        (
            "ADMIN",
            color::ON_PRIMARY_CONTAINER,
            theme::with_alpha(color::SUCCESS, 0.22),
        )
    } else {
        (
            "USER",
            color::ON_SURFACE_VARIANT,
            color::SURFACE_CONTAINER_HIGHEST,
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
            egui::Stroke::new(1.0, theme::with_alpha(color::SUCCESS, 0.55)),
        );
    } else {
        painter.rect_stroke(
            rect,
            rounding,
            egui::Stroke::new(1.0, color::OUTLINE_VARIANT),
        );
    }
    painter.galley(rect.left_top() + egui::vec2(pad_x, pad_y), galley, fg);
}

fn run_card(ui: &mut egui::Ui, app: &mut App, card_width: f32) {
    let running = app.proc.is_running();

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
                (color::SUCCESS, "Running")
            } else {
                (color::ON_SURFACE_VARIANT, "Stopped")
            };
            status_text_block(ui, status_text, status_color);
            // Right-align the process integrity chip on the status row.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                elevation_pill(ui);
            });
        });

        if let Some(err) = app.last_error.clone() {
            ui.add_space(8.0);
            banner(ui, &err, color::ERROR_CONTAINER, color::ERROR);
        }

        ui.add_space(14.0);

        const LOG_INNER_MARGIN: f32 = 10.0;
        const LOG_HEIGHT: f32 = 200.0;
        let log_inner_width = (ui.available_width() - LOG_INNER_MARGIN * 2.0).max(0.0);

        egui::Frame::none()
            .fill(color::SURFACE_CONTAINER_LOWEST)
            .rounding(egui::Rounding::same(radius::MD))
            .inner_margin(egui::Margin::same(LOG_INNER_MARGIN))
            .show(ui, |ui| {
                ui.set_min_width(log_inner_width);
                ui.set_max_width(log_inner_width);
                ui.set_min_height(LOG_HEIGHT);

                // The log view has dense monospace text. Keep a reserved
                // gutter so scrollbars never cover text, but let the handle
                // itself auto-hide when idle.
                {
                    let scroll = &mut ui.spacing_mut().scroll;
                    scroll.floating = true;
                    scroll.floating_allocated_width = 10.0;
                    scroll.floating_width = 3.0;
                    scroll.bar_width = 6.0;
                    scroll.dormant_background_opacity = 0.0;
                    scroll.dormant_handle_opacity = 0.0;
                    scroll.active_background_opacity = 0.0;
                    scroll.active_handle_opacity = 0.65;
                    scroll.interact_background_opacity = 0.0;
                    scroll.interact_handle_opacity = 0.9;
                }

                egui::ScrollArea::both()
                    .auto_shrink([false; 2])
                    .max_height(LOG_HEIGHT)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        if app.logs.is_empty() {
                            ui.label(
                                egui::RichText::new("(no logs yet)")
                                    .color(color::ON_SURFACE_VARIANT),
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
    });
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
            color::ON_SURFACE_VARIANT,
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
        color::ON_SURFACE_VARIANT,
    );
    painter.galley(
        rect.left_top() + egui::vec2(0.0, y - rect.top() + caption_h + line_gap),
        status,
        status_color,
    );
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

/// A field caption in M3 "Label Small" style — small, muted, all-caps.
fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(color::ON_SURFACE_VARIANT)
            .size(11.0)
            .strong(),
    );
}

/// Inline validation message shown directly under a form input.
/// Sized to match `field_label` (Label Small) so it doesn't visually
/// outweigh the field itself, and tinted with the M3 error color.
fn field_error(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).color(color::ERROR).size(11.0));
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
            painter.rect_filled(seg_rect, seg_rounding, color::SECONDARY_CONTAINER);
        } else if resp.hovered() {
            painter.rect_filled(
                seg_rect,
                seg_rounding,
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

    let result = theme::modal_dialog(ctx, "add_config_modal", title, 320.0, !busy, |ui| {
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
                app.add_dialog.reset();
            }
        });
    });

    if result.close_requested {
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

    let result = theme::modal_dialog(
        ctx,
        "delete_confirm_modal",
        "Delete config",
        360.0,
        true,
        |ui| {
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
                egui::RichText::new("This cannot be undone.").color(color::ON_SURFACE_VARIANT),
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
    let result = theme::modal_dialog(
        ctx,
        "destroy_confirm_modal",
        "Destroy working directory",
        360.0,
        true,
        |ui| {
            ui.label(
                egui::RichText::new(
                    "This will clear the current working directory. \
                     This action cannot be undone.",
                )
                .color(color::ON_SURFACE),
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
        },
    );
    if result.close_requested {
        app.destroy_confirm_open = false;
    }
}
