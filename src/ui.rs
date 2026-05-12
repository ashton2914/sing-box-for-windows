use eframe::egui;

use crate::app::{App, BgCmd, SourceKind};
use crate::config::entry::Source;
use crate::core::shell;
use crate::theme::{self, color, radius};

pub fn show(ui: &mut egui::Ui, app: &mut App) {
    egui::Frame::none()
        .inner_margin(egui::Margin {
            left: 16.0,
            right: 16.0,
            top: 12.0,
            bottom: 12.0,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("sing-box")
                        .color(color::ON_SURFACE)
                        .size(20.0)
                        .strong(),
                );
            });
            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    run_card(ui, app);
                    ui.add_space(8.0);
                    config_card(ui, app);
                    ui.add_space(8.0);
                    settings_card(ui, app);
                });
        });

    add_config_modal(ui.ctx(), app);
    delete_confirm_modal(ui.ctx(), app);
    destroy_confirm_modal(ui.ctx(), app);
}

// ---------- Config card ----------

fn config_card(ui: &mut egui::Ui, app: &mut App) {
    theme::card(ui, |ui| {
        ui.horizontal(|ui| {
            theme::section_title(ui, "Configs");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(theme::filled_button("+ Add")).clicked() {
                    app.add_dialog.reset();
                    app.add_dialog.open = true;
                }
                if ui
                    .add(theme::icon_button("⟳"))
                    .on_hover_text("Refresh list")
                    .clicked()
                {
                    app.refresh_listings();
                }
            });
        });
        ui.add_space(6.0);

        // ----- selector -----
        let selected_text = match app.selected_entry() {
            Some(e) => e.metadata.name.clone(),
            None => "(none)".to_string(),
        };
        let mut new_sel = app.settings.selected_config.clone();
        egui::ComboBox::from_id_source("config_combo")
            .width(ui.available_width())
            .selected_text(&selected_text)
            .show_ui(ui, |ui| {
                if app.configs.is_empty() {
                    ui.label(
                        egui::RichText::new("(no configs — click + Add)")
                            .color(color::ON_SURFACE_VARIANT)
                            .italics(),
                    );
                }
                for c in &app.configs {
                    let label = format!(
                        "{}    [{}]",
                        c.metadata.name,
                        c.metadata.source.kind_label()
                    );
                    ui.selectable_value(&mut new_sel, Some(c.slug.clone()), label);
                }
            });
        if new_sel != app.settings.selected_config {
            app.settings.selected_config = new_sel;
            app.persist_settings();
        }

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
            let (label, hover) = if running {
                ("⏹", "Stop sing-box")
            } else {
                ("▶", "Start sing-box")
            };
            if ui.add(theme::fab(label)).on_hover_text(hover).clicked() {
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
        });

        if let Some(err) = app.last_error.clone() {
            ui.add_space(8.0);
            banner(ui, &err, color::ERROR_CONTAINER, color::ERROR);
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            theme::subsection_title(ui, "LOG");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if app.log_visible { "Hide log" } else { "Show log" };
                if ui.add(theme::text_button(label)).clicked() {
                    app.log_visible = !app.log_visible;
                }
            });
        });
        if app.log_visible {
            ui.add_space(4.0);
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
        .inner_margin(egui::Margin::same(20.0))
        .stroke(egui::Stroke::new(1.0, color::OUTLINE_VARIANT))
        .shadow(egui::epaint::Shadow {
            offset: egui::vec2(0.0, 8.0),
            blur: 24.0,
            spread: 0.0,
            color: egui::Color32::from_black_alpha(120),
        })
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
            .size(18.0)
            .strong(),
    )
    .collapsible(false)
    .resizable(false)
    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
    .frame(modal_frame())
    .open(&mut still_open)
    .show(ctx, |ui| {
        ui.set_min_width(380.0);

        // Name
        ui.label(
            egui::RichText::new("Name")
                .color(color::ON_SURFACE_VARIANT)
                .size(12.0)
                .strong(),
        );
        ui.add(
            egui::TextEdit::singleline(&mut app.add_dialog.name)
                .hint_text("My Subscription")
                .desired_width(f32::INFINITY)
                .margin(egui::vec2(10.0, 8.0)),
        );

        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("Source")
                .color(color::ON_SURFACE_VARIANT)
                .size(12.0)
                .strong(),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut app.add_dialog.kind,
                SourceKind::Remote,
                "Remote (URL)",
            );
            ui.add_space(8.0);
            ui.radio_value(
                &mut app.add_dialog.kind,
                SourceKind::Local,
                "Local (file)",
            );
        });

        ui.add_space(8.0);
        match app.add_dialog.kind {
            SourceKind::Remote => {
                ui.label(
                    egui::RichText::new("URL")
                        .color(color::ON_SURFACE_VARIANT)
                        .size(12.0)
                        .strong(),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut app.add_dialog.url)
                        .hint_text("https://example.com/sub.json")
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(10.0, 8.0)),
                );
            }
            SourceKind::Local => {
                ui.label(
                    egui::RichText::new("File")
                        .color(color::ON_SURFACE_VARIANT)
                        .size(12.0)
                        .strong(),
                );
                ui.horizontal(|ui| {
                    let avail = ui.available_width() - 96.0;
                    ui.add_sized(
                        [avail.max(120.0), 36.0],
                        egui::TextEdit::singleline(&mut app.add_dialog.path)
                            .hint_text(r"C:\path\to\config.json")
                            .margin(egui::vec2(10.0, 8.0)),
                    );
                    if ui.add(theme::tonal_button("Browse…")).clicked() {
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

        ui.add_space(16.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let busy = app.add_dialog.busy;
            let primary_label = if busy {
                if is_edit { "Saving…" } else { "Adding…" }
            } else if is_edit {
                "Save"
            } else {
                "Add"
            };
            let primary_btn = ui.add_enabled(!busy, theme::filled_button(primary_label));
            if primary_btn.clicked() {
                app.submit_add_dialog();
            }
            ui.add_space(8.0);
            if ui.add_enabled(!busy, theme::text_button("Cancel")).clicked() {
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
