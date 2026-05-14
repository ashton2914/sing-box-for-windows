use eframe::egui;

use crate::app::{App, BgCmd, SourceKind};
use crate::config::entry::Source;
use crate::core::shell;
use crate::theme::{self, color, radius};

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

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    egui::Frame::none()
                        .inner_margin(egui::Margin {
                            left: PAGE_INSET,
                            right: PAGE_INSET,
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
    about_modal(ui.ctx(), app);
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

    let outline = egui::Stroke::new(1.0, color::OUTLINE_VARIANT);
    let surface = color::SURFACE_CONTAINER;

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
        theme::with_alpha(color::PRIMARY, 0.14)
    } else if resp.hovered() {
        theme::with_alpha(color::ON_SURFACE, 0.06)
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
    let selected_summary = app
        .selected_entry()
        .map(|e| (e.metadata.name.clone(), e.metadata.source.kind_label()));

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
                let name_galley =
                    ui.fonts(|f| f.layout_no_wrap(name.clone(), name_font, color::ON_SURFACE));
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
                    let _ = theme::paint_chip(ui, painter, chip_center, kind);
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
        },
        |ui| {
            if configs_snapshot.is_empty() {
                ui.label(
                    egui::RichText::new("(no configs — click + Add)")
                        .color(color::ON_SURFACE_VARIANT),
                );
                return None;
            }
            for (slug, name, kind) in &configs_snapshot {
                let selected = current_selected.as_deref() == Some(slug.as_str());
                let clicked = popup_combo_row(ui, 32.0, selected, |ui, p, inner| {
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
                    let chip_center = egui::pos2(inner.left() + ns.x + 8.0 + cs.x * 0.5, cy);
                    let _ = theme::paint_chip(ui, p, chip_center, kind);
                });
                if clicked {
                    return Some(slug.clone());
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
    let cores_snapshot: Vec<String> = app.cores.clone();
    let current = app.settings.selected_core.clone();
    let current_label = current.clone();

    let picked = popup_combo(
        ui,
        "core_combo_v1",
        width,
        32.0,
        |ui, painter, inner, _content_right| {
            let name_font = egui::FontId::proportional(13.5);
            let (text, color_) = match current_label.as_deref() {
                Some(name) => (name.to_string(), color::ON_SURFACE),
                None => ("(none)".to_string(), color::ON_SURFACE_VARIANT),
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
            if cores_snapshot.is_empty() {
                ui.label(
                    egui::RichText::new("(no *.exe under core/)").color(color::ON_SURFACE_VARIANT),
                );
                return None;
            }
            for name in &cores_snapshot {
                let selected = current.as_deref() == Some(name.as_str());
                let clicked = popup_combo_row(ui, 28.0, selected, |ui, p, inner| {
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
                });
                if clicked {
                    return Some(Some(name.clone()));
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
                ui.horizontal(|ui| {
                    let label = ui.label(theme::setting_label(crate::core::loopback::DISPLAY_NAME));
                    label.on_hover_text(crate::core::loopback::HOVER_TEXT);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Keep this as a compact, low-emphasis row action
                        // rather than a full tonal pill. The fixed width
                        // still prevents right-to-left reflow when the
                        // label flips "Open" ↔ "Opening…".
                        let label = if app.loopback_busy {
                            "Opening\u{2026}"
                        } else {
                            "Open"
                        };
                        let loopback_button = egui::Button::new(
                            egui::RichText::new(label)
                                .color(color::ON_SURFACE)
                                .size(12.5),
                        )
                        .fill(color::SURFACE_CONTAINER_HIGH)
                        .rounding(egui::Rounding::same(radius::FULL))
                        .min_size(egui::Vec2::new(76.0, 24.0))
                        .stroke(egui::Stroke::new(1.0, color::OUTLINE_VARIANT));
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
                        egui::RichText::new(env!("CARGO_PKG_VERSION")).color(color::ON_SURFACE),
                    );
                });
            });
            ui.horizontal(|ui| {
                ui.label(theme::setting_label("Copyright"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("© 2026 ashton2914").color(color::ON_SURFACE));
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
                        app.about_open = true;
                    }
                });
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
            if banner(ui, &err, color::ERROR_CONTAINER, color::ERROR) {
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
                theme::with_alpha(color::SUCCESS, 0.18),
                color::ON_SURFACE,
            ) {
                app.last_info = None;
            }
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
            theme::with_alpha(color::ON_SURFACE, 0.12),
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
                theme::request_modal_close(ctx, "add_config_modal");
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
                    theme::request_modal_close(ctx, "delete_confirm_modal");
                }
                ui.add_space(8.0);
                if ui.add(theme::text_button("Cancel")).clicked() {
                    theme::request_modal_close(ctx, "delete_confirm_modal");
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
                    theme::request_modal_close(ctx, "destroy_confirm_modal");
                }
                ui.add_space(8.0);
                if ui.add(theme::text_button("Cancel")).clicked() {
                    theme::request_modal_close(ctx, "destroy_confirm_modal");
                }
            });
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

    let result = theme::modal_dialog_sized(
        ctx,
        "about_modal",
        "About",
        modal_w,
        body_max_h,
        frame_pad,
        true,
        |ui, body_max_h| {
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
                        .color(color::ON_SURFACE)
                        .strong()
                        .size(15.0),
                    );
                    ui.label(
                        egui::RichText::new("Copyright © 2026 ashton2914")
                            .color(color::ON_SURFACE_VARIANT),
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
                        .color(color::ON_SURFACE),
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
                        .color(color::ON_SURFACE),
                    );

                    ui.add_space(8.0);

                    // -- Full GPLv3 text in a compact monospace box.
                    // Its own ScrollArea so the user can browse the
                    // 35KB license without dragging the outer scroll
                    // hundreds of pixels. The inner area's height is
                    // intentionally short to keep the overall modal
                    // tidy.
                    egui::Frame::none()
                        .fill(color::SURFACE_CONTAINER_LOWEST)
                        .rounding(egui::Rounding::same(radius::MD))
                        .inner_margin(egui::Margin::same(10.0))
                        .show(ui, |ui| {
                            let inner_width = (ui.available_width()).max(0.0);
                            ui.set_min_width(inner_width);
                            ui.set_max_width(inner_width);
                            egui::ScrollArea::vertical()
                                .auto_shrink([false; 2])
                                .max_height(180.0)
                                .id_source("about_license_scroll")
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(LICENSE_TEXT)
                                                .color(color::ON_SURFACE_VARIANT)
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
                    theme::request_modal_close(ctx, "about_modal");
                }
            });
        },
    );
    if result.close_requested {
        app.about_open = false;
    }
}
