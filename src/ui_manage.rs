//! Vistas de gestión de GitHub: PR, Issues, Actions y Releases.
//! Estas pantallas consultan o modifican datos remotos; las peticiones se hacen en segundo plano.

use crate::app::GitCraftApp;
use eframe::egui;
use egui_extras::{Column, TableBuilder};

fn format_action_date(iso: &str) -> String {
    if iso.len() >= 16 {
        iso[..16].replace('T', " ")
    } else if iso.is_empty() {
        "-".to_string()
    } else {
        iso.to_string()
    }
}

// Dibuja la pestaña de Pull Requests (listar, crear y hacer merge).
pub fn render_prs_tab(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    if app.selected_repo.is_none() { ui.label(app.i18n.t(lang, "no_repo_selected")); return; }

    ui.horizontal_wrapped(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.new_pr_title).hint_text(app.i18n.t(lang, "pr_new_title")));

        // Desplegable de Rama origen (head)
        ui.label(app.i18n.t(lang, "pr_head_branch"));
        egui::ComboBox::from_id_salt("pr_head_combo")
            .selected_text(&app.new_pr_head)
            .show_ui(ui, |ui| {
                for b in &app.branches.clone() {
                    ui.selectable_value(&mut app.new_pr_head, b.name.clone(), &b.name);
                }
            });

        // Desplegable de Rama destino (base)
        ui.label(app.i18n.t(lang, "pr_base_branch"));
        egui::ComboBox::from_id_salt("pr_base_combo")
            .selected_text(&app.new_pr_base)
            .show_ui(ui, |ui| {
                for b in &app.branches.clone() {
                    ui.selectable_value(&mut app.new_pr_base, b.name.clone(), &b.name);
                }
            });

        // Solo permitimos crear el PR si hay título, ambas ramas están elegidas y son distintas entre sí
        let can_create = !app.new_pr_title.trim().is_empty()
            && !app.new_pr_head.is_empty()
            && !app.new_pr_base.is_empty()
            && app.new_pr_head != app.new_pr_base;

        if ui.add_enabled(can_create, egui::Button::new(app.i18n.t(lang, "pr_create_btn"))).clicked() {
            app.trigger_create_pr(ctx.clone());
        }
        if ui.button(app.i18n.t(lang, "refresh_list_btn")).clicked() {
            // Refrescamos PRs y ramas sin cambiar la rama activa.
            app.refresh_branches(ctx.clone());
            app.load_management(ctx.clone());
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        for pr in app.mgmt_data.prs.clone() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("#{} {}", pr.number, pr.title)).strong());
                    if ui.button(app.i18n.t(lang, "pr_merge_btn")).clicked() {
                        app.trigger_merge_pr(ctx.clone(), pr.number);
                    }
                });
            });
        }
    });
}

// Dibuja la pestaña de Issues (listar, crear y cerrar).
pub fn render_issues_tab(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    if app.selected_repo.is_none() { ui.label(app.i18n.t(lang, "no_repo_selected")); return; }

    ui.horizontal_wrapped(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.new_issue_title).hint_text(app.i18n.t(lang, "issue_new_title")));
        ui.add(egui::TextEdit::singleline(&mut app.new_issue_body).hint_text(app.i18n.t(lang, "issue_new_body")));
        if ui.button(app.i18n.t(lang, "issue_create_btn")).clicked() && !app.new_issue_title.trim().is_empty() {
            app.trigger_create_issue(ctx.clone());
        }
        if ui.button(app.i18n.t(lang, "refresh_list_btn")).clicked() { app.load_management(ctx.clone()); }
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        for issue in app.mgmt_data.issues.clone() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("#{} {}", issue.number, issue.title)).strong());
                    if ui.button(app.i18n.t(lang, "issue_close_btn")).clicked() {
                        app.trigger_close_issue(ctx.clone(), issue.number);
                    }
                });
            });
        }
    });
}

// Dibuja la pestaña de GitHub Actions con tabla de 6 columnas: OK/KO, Workflow, Usuario, Rama, Fecha y Relanzar.
pub fn render_actions_tab(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    if app.selected_repo.is_none() {
        ui.label(app.i18n.t(lang, "no_repo_selected"));
        return;
    }

    if ui.button(app.i18n.t(lang, "refresh_list_btn")).clicked() {
        app.load_management(ctx.clone());
    }
    ui.add_space(6.0);
    ui.separator();

    TableBuilder::new(ui)
        .id_salt("actions_runs_table")
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(75.0).at_least(65.0))   // 1. OK / KO
        .column(Column::remainder().at_least(150.0))    // 2. Workflow
        .column(Column::initial(120.0).at_least(90.0))  // 3. Usuario
        .column(Column::initial(110.0).at_least(85.0))  // 4. Rama
        .column(Column::initial(135.0).at_least(115.0)) // 5. Fecha
        .column(Column::initial(110.0).at_least(95.0))  // 6. Botón relanzar
        .header(26.0, |mut header| {
            header.col(|ui| { ui.strong(app.i18n.t(lang, "col_act_status")); });
            header.col(|ui| { ui.strong(app.i18n.t(lang, "col_act_workflow")); });
            header.col(|ui| { ui.strong(app.i18n.t(lang, "col_act_user")); });
            header.col(|ui| { ui.strong(app.i18n.t(lang, "col_act_branch")); });
            header.col(|ui| { ui.strong(app.i18n.t(lang, "col_act_date")); });
            header.col(|ui| { ui.strong(app.i18n.t(lang, "col_act_action")); });
        })
        .body(|mut body| {
            for run in app.mgmt_data.runs.clone() {
                body.row(26.0, |mut row| {
                    // Columna 1: OK / KO con color distintivo
                    row.col(|ui| {
                        match run.conclusion.as_deref() {
                            Some("success") => {
                                ui.colored_label(egui::Color32::from_rgb(92, 184, 92), egui::RichText::new("OK").strong());
                            }
                            Some("failure") | Some("cancelled") | Some("timed_out") => {
                                ui.colored_label(egui::Color32::from_rgb(217, 83, 79), egui::RichText::new("KO").strong());
                            }
                            _ => {
                                ui.colored_label(egui::Color32::from_rgb(240, 173, 78), egui::RichText::new(&run.status).strong());
                            }
                        }
                    });

                    // Columna 2: Nombre del Workflow
                    row.col(|ui| {
                        ui.add(egui::Label::new(egui::RichText::new(&run.name).strong()).truncate());
                    });

                    // Columna 3: Usuario que disparó la ejecución
                    row.col(|ui| {
                        let user_display = if run.actor.login.is_empty() {
                            "-".to_string()
                        } else {
                            format!("@{}", run.actor.login)
                        };
                        ui.label(user_display);
                    });

                    // Columna 4: Rama
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&run.head_branch).monospace());
                    });

                    // Columna 5: Fecha de ejecución
                    row.col(|ui| {
                        ui.label(format_action_date(&run.created_at));
                    });

                    // Columna 6: Botón relanzar
                    row.col(|ui| {
                        if ui.button(app.i18n.t(lang, "actions_rerun_btn")).clicked() {
                            app.trigger_rerun_workflow(ctx.clone(), run.id);
                        }
                    });
                });
            }
        });
}

// Dibuja la pestaña de Releases (publicar y ver versiones).
pub fn render_releases_tab(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    if app.selected_repo.is_none() { ui.label(app.i18n.t(lang, "no_repo_selected")); return; }

    ui.horizontal_wrapped(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.new_rel_tag).desired_width(120.0).hint_text(app.i18n.t(lang, "release_tag_hint")));
        ui.add(egui::TextEdit::singleline(&mut app.new_rel_name).hint_text(app.i18n.t(lang, "release_name_hint")));
        ui.add(egui::TextEdit::singleline(&mut app.new_rel_body).hint_text(app.i18n.t(lang, "release_notes_hint")));
        if ui.button(app.i18n.t(lang, "release_create_btn")).clicked() && !app.new_rel_tag.trim().is_empty() {
            app.trigger_create_release(ctx.clone());
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        for rel in app.mgmt_data.releases.clone() {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(egui::RichText::new(format!("🏷 {} - {}", rel.tag_name, rel.name.unwrap_or_default())).strong());
                if let Some(b) = rel.body { ui.label(egui::RichText::new(b).weak()); }
            });
        }
    });
}
