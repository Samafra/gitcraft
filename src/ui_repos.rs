use crate::app::{ActiveTab, GitCraftApp};
use eframe::egui;
use egui_extras::{Column, TableBuilder};

// Función auxiliar para formatear fechas ISO de GitHub ("2026-11-14T18:30:00Z") a "2026-11-14 18:30".
fn format_github_date(iso: &str) -> String {
    if iso.len() >= 16 {
        iso[..16].replace('T', " ")
    } else if iso.is_empty() {
        "-".to_string()
    } else {
        iso.to_string()
    }
}

// Dibuja la pestaña de Repositorios con buscador y tabla de 5 columnas.
pub fn render_repos_tab(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;

    // Barra superior de búsqueda y acciones
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.repo_filter)
                .desired_width(260.0)
                .hint_text(app.i18n.t(lang, "repos_search_hint")),
        );
        if ui.button(app.i18n.t(lang, "repos_refresh_btn")).clicked() {
            app.trigger_github_sync(ctx.clone());
        }
        if ui.button(app.i18n.t(lang, "auth_disconnect_btn")).clicked() {
            app.user = None;
            app.repos.clear();
            app.saved_token.clear();
        }
    });

    ui.add_space(6.0);
    ui.separator();

    // Filtramos los repositorios según el texto escrito en el buscador
    let filter = app.repo_filter.to_lowercase();
    let filtered_repos: Vec<_> = app
        .repos
        .iter()
        .filter(|r| {
            filter.is_empty()
                || r.full_name.to_lowercase().contains(&filter)
                || r.description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&filter)
        })
        .cloned()
        .collect();

    // Construimos la tabla de 5 columnas usando TableBuilder de egui_extras
    TableBuilder::new(ui)
        .striped(true) // Alterna el color de fondo de las filas para facilitar la lectura
        .resizable(true) // Permite arrastrar el borde de cada columna para ajustarla
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(220.0).at_least(140.0)) // 1. Nombre repositorio
        .column(Column::initial(115.0).at_least(90.0))  // 2. Público / Privado
        .column(Column::remainder().at_least(180.0))    // 3. Descripción (ocupa el espacio restante)
        .column(Column::initial(135.0).at_least(110.0)) // 4. Fecha creación
        .column(Column::initial(135.0).at_least(110.0)) // 5. Fecha última modificación
        .header(26.0, |mut header| {
            header.col(|ui| {
                ui.strong(app.i18n.t(lang, "col_repo_name"));
            });
            header.col(|ui| {
                ui.strong(app.i18n.t(lang, "col_visibility"));
            });
            header.col(|ui| {
                ui.strong(app.i18n.t(lang, "col_description"));
            });
            header.col(|ui| {
                ui.strong(app.i18n.t(lang, "col_created_at"));
            });
            header.col(|ui| {
                ui.strong(app.i18n.t(lang, "col_updated_at"));
            });
        })
        .body(|mut body| {
            for repo in filtered_repos {
                let is_selected = app
                    .selected_repo
                    .as_ref()
                    .map(|r| r.id == repo.id)
                    .unwrap_or(false);

                body.row(24.0, |mut row| {
                    // Columna 1: Nombre del repositorio (clickable para abrirlo)
                    row.col(|ui| {
                        let label = egui::RichText::new(&repo.full_name).strong();
                        if ui.selectable_label(is_selected, label).clicked() {
                            app.selected_repo = Some(repo.clone());
                            app.active_tab = ActiveTab::Changes;
                            app.load_branches_and_root(ctx.clone());
                        }
                    });

                    // Columna 2: Público / Privado con color distintivo
                    row.col(|ui| {
                        if repo.private {
                            ui.colored_label(
                                egui::Color32::from_rgb(240, 173, 78),
                                format!("🔒 {}", app.i18n.t(lang, "repos_private_badge")),
                            );
                        } else {
                            ui.colored_label(
                                egui::Color32::from_rgb(92, 184, 92),
                                format!("🌐 {}", app.i18n.t(lang, "repos_public_badge")),
                            );
                        }
                    });

                    // Columna 3: Descripción
                    row.col(|ui| {
                        let desc = repo.description.as_deref().unwrap_or("-");
                        ui.add(egui::Label::new(egui::RichText::new(desc).weak()).truncate());
                    });

                    // Columna 4: Fecha de creación
                    row.col(|ui| {
                        ui.label(format_github_date(&repo.created_at));
                    });

                    // Columna 5: Fecha de última modificación
                    row.col(|ui| {
                        ui.label(format_github_date(&repo.updated_at));
                    });
                });
            }
        });
}
