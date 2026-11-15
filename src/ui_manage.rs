use crate::app::GitCraftApp;
use eframe::egui;

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
            app.load_branches_and_root(ctx.clone());
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

// Dibuja la pestaña de GitHub Actions (historial de workflows y relanzamiento).
pub fn render_actions_tab(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    if app.selected_repo.is_none() { ui.label(app.i18n.t(lang, "no_repo_selected")); return; }

    if ui.button(app.i18n.t(lang, "refresh_list_btn")).clicked() { app.load_management(ctx.clone()); }
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        for run in app.mgmt_data.runs.clone() {
            let badge = match run.conclusion.as_deref() {
                Some("success") => "✅",
                Some("failure") => "❌",
                _ => "⏳",
            };
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{} {} ({}) [{}]", badge, run.name, run.head_branch, run.status));
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
