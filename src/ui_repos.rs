use crate::app::{ActiveTab, GitCraftApp, RepoSortColumn};
use eframe::egui;
use egui_extras::{Column, TableBuilder};

// Formatea fechas ISO de GitHub ("2026-11-14T18:30:00Z") a "2026-11-14 18:30".
fn format_github_date(iso: &str) -> String {
    if iso.len() >= 16 {
        iso[..16].replace('T', " ")
    } else if iso.is_empty() {
        "-".to_string()
    } else {
        iso.to_string()
    }
}

// Dibuja la cabecera clickable y pinta un triángulo vectorial nativo (arriba/abajo) en la columna activa.
fn sort_header_button(ui: &mut egui::Ui, label: &str, col: RepoSortColumn, app: &mut GitCraftApp) {
    let is_active = app.repo_sort_col == col;
    let text = egui::RichText::new(label).strong();

    ui.horizontal(|ui| {
        if ui.selectable_label(is_active, text).clicked() {
            if is_active {
                app.repo_sort_asc = !app.repo_sort_asc;
            } else {
                app.repo_sort_col = col;
                app.repo_sort_asc = !matches!(col, RepoSortColumn::CreatedAt | RepoSortColumn::UpdatedAt);
            }
        }

        // Si esta columna está activa, dibujamos un triángulo vectorial de 8x5 px junto al título.
        if is_active {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 14.0), egui::Sense::hover());
            let c = rect.center();
            let color = ui.visuals().strong_text_color();
            let points = if app.repo_sort_asc {
                // Triángulo apuntando hacia arriba
                vec![
                    egui::pos2(c.x, c.y - 3.0),
                    egui::pos2(c.x - 4.0, c.y + 3.0),
                    egui::pos2(c.x + 4.0, c.y + 3.0),
                ]
            } else {
                // Triángulo apuntando hacia abajo
                vec![
                    egui::pos2(c.x - 4.0, c.y - 3.0),
                    egui::pos2(c.x + 4.0, c.y - 3.0),
                    egui::pos2(c.x, c.y + 3.0),
                ]
            };
            ui.painter().add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
        }
    });
}

// Dibuja la pestaña de Repositorios con buscador, ordenación por columnas y tabla de 5 columnas.
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

    // 1. Filtramos los repositorios según el texto del buscador
    let filter = app.repo_filter.to_lowercase();
    let mut filtered_repos: Vec<_> = app
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

    // 2. Ordenamos la lista según la columna y dirección seleccionadas
    let sort_col = app.repo_sort_col;
    let sort_asc = app.repo_sort_asc;
    filtered_repos.sort_by(|a, b| {
        let ord = match sort_col {
            RepoSortColumn::Name => a.full_name.to_lowercase().cmp(&b.full_name.to_lowercase()),
            RepoSortColumn::Visibility => a.private.cmp(&b.private),
            RepoSortColumn::Description => a
                .description
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .cmp(&b.description.as_deref().unwrap_or("").to_lowercase()),
            RepoSortColumn::CreatedAt => a.created_at.cmp(&b.created_at),
            RepoSortColumn::UpdatedAt => a.updated_at.cmp(&b.updated_at),
        };
        if sort_asc { ord } else { ord.reverse() }
    });

    // 3. Construimos la tabla de 5 columnas con cabeceras clickables
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(220.0).at_least(140.0)) // 1. Nombre repositorio
        .column(Column::initial(125.0).at_least(95.0))  // 2. Público / Privado
        .column(Column::remainder().at_least(180.0))    // 3. Descripción
        .column(Column::initial(145.0).at_least(115.0)) // 4. Fecha creación
        .column(Column::initial(155.0).at_least(120.0)) // 5. Última modificación
        .header(26.0, |mut header| {
            let lbl_name = app.i18n.t(lang, "col_repo_name").to_string();
            let lbl_vis = app.i18n.t(lang, "col_visibility").to_string();
            let lbl_desc = app.i18n.t(lang, "col_description").to_string();
            let lbl_created = app.i18n.t(lang, "col_created_at").to_string();
            let lbl_updated = app.i18n.t(lang, "col_updated_at").to_string();

            header.col(|ui| sort_header_button(ui, &lbl_name, RepoSortColumn::Name, app));
            header.col(|ui| sort_header_button(ui, &lbl_vis, RepoSortColumn::Visibility, app));
            header.col(|ui| sort_header_button(ui, &lbl_desc, RepoSortColumn::Description, app));
            header.col(|ui| sort_header_button(ui, &lbl_created, RepoSortColumn::CreatedAt, app));
            header.col(|ui| sort_header_button(ui, &lbl_updated, RepoSortColumn::UpdatedAt, app));
        })
        .body(|mut body| {
            for repo in filtered_repos {
                let is_selected = app
                    .selected_repo
                    .as_ref()
                    .map(|r| r.id == repo.id)
                    .unwrap_or(false);

                body.row(24.0, |mut row| {
                    row.col(|ui| {
                        let label = egui::RichText::new(&repo.full_name).strong();
                        if ui.selectable_label(is_selected, label).clicked() {
                            app.selected_repo = Some(repo.clone());
                            app.active_tab = ActiveTab::Changes;
                            app.load_branches_and_root(ctx.clone());
                        }
                    });

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

                    row.col(|ui| {
                        let desc = repo.description.as_deref().unwrap_or("-");
                        ui.add(egui::Label::new(egui::RichText::new(desc).weak()).truncate());
                    });

                    row.col(|ui| {
                        ui.label(format_github_date(&repo.created_at));
                    });

                    row.col(|ui| {
                        ui.label(format_github_date(&repo.updated_at));
                    });
                });
            }
        });
}
