use crate::app::GitCraftApp;
use eframe::egui;
use egui_extras::{Column, TableBuilder};

// Dibuja el contenido de la pestaña "Cambios y Ramas" con tabla de archivos de 4 columnas sin cabecera.
pub fn render_changes_tab(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;

    let Some(repo) = app.selected_repo.clone() else {
        ui.add_space(20.0);
        ui.label(egui::RichText::new(app.i18n.t(lang, "no_repo_selected")).size(16.0));
        return;
    };

    // Barra superior de gestión de ramas
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(app.i18n.t(lang, "branch_label")).strong());

        egui::ComboBox::from_id_salt("branch_selector")
            .selected_text(&app.current_branch)
            .show_ui(ui, |ui| {
                for b in &app.branches.clone() {
                    if ui.selectable_value(&mut app.current_branch, b.name.clone(), &b.name).clicked() {
                        // La petición anterior y el editor ya no pertenecen a esta rama.
                        app.current_path.clear();
                        app.opened_file_path = None;
                        app.opened_file_sha.clear();
                        app.opened_file_content.clear();
                        app.file_request = None;
                        app.load_directory(ctx.clone());
                    }
                }
            });

        if ui.button("🔄").clicked() {
            // Refrescar no debe mover el usuario a la rama predeterminada.
            app.refresh_branches(ctx.clone());
            app.load_directory(ctx.clone());
        }

        ui.separator();
        ui.add(egui::TextEdit::singleline(&mut app.new_branch_name).hint_text(app.i18n.t(lang, "new_branch_hint")));
        if ui.button(app.i18n.t(lang, "create_branch_btn")).clicked() && !app.new_branch_name.trim().is_empty() {
            app.trigger_create_branch(ctx.clone());
        }
    });

    ui.separator();

    // Si hay un archivo abierto, dividimos la pantalla en 2 columnas; si no, la tabla ocupa el 100 % del ancho.
    if let Some(file_path) = app.opened_file_path.clone() {
        ui.columns(2, |cols| {
            // COLUMNA IZQUIERDA (cols[0]): Tabla de archivos
            render_files_column(app, &mut cols[0], ctx, &repo.name);

            // COLUMNA DERECHA (cols[1]): Editor con botón para cerrar
            cols[1].horizontal(|ui| {
                ui.heading(format!("📄 {}", file_path));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✖").on_hover_text("Cerrar archivo").clicked() {
                        app.opened_file_path = None;
                        app.opened_file_content.clear();
                        app.opened_file_sha.clear();
                        app.file_request = None;
                    }
                });
            });
            cols[1].separator();

            egui::ScrollArea::vertical().id_salt("editor_scroll").max_height(360.0).show(&mut cols[1], |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut app.opened_file_content)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(18),
                );
            });

            cols[1].add_space(8.0);
            cols[1].add(
                egui::TextEdit::singleline(&mut app.commit_message)
                    .desired_width(f32::INFINITY)
                    .hint_text(app.i18n.t(lang, "commit_msg_hint")),
            );
            cols[1].add_space(4.0);
            if cols[1].button(app.i18n.t(lang, "commit_push_btn")).clicked() {
                app.trigger_commit_push(ctx.clone());
            }
        });
    } else {
        // Sin archivo abierto: la tabla de archivos ocupa todo el ancho disponible
        render_files_column(app, ui, ctx, &repo.name);
    }
}

// Función auxiliar que dibuja la cabecera de carpeta y la tabla de archivos de 4 columnas sin cabecera.
fn render_files_column(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context, repo_name: &str) {
    let lang = app.language;
    ui.horizontal(|ui| {
        ui.heading(app.i18n.t(lang, "files_header"));
        if !app.current_path.is_empty() && ui.button(app.i18n.t(lang, "back_folder_btn")).clicked() {
            if let Some((parent, _)) = app.current_path.rsplit_once('/') {
                app.current_path = parent.to_string();
            } else {
                app.current_path.clear();
            }
            app.load_directory(ctx.clone());
        }
    });
    ui.label(egui::RichText::new(format!("/{}/{}", repo_name, app.current_path)).weak());
    ui.separator();

    TableBuilder::new(ui)
        .id_salt("repo_files_table")
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(170.0).at_least(110.0)) // 1. Nombre archivo/carpeta
        .column(Column::remainder().at_least(130.0))    // 2. Descripción del commit
        .column(Column::initial(65.0).at_least(55.0))   // 3. Hash corto del commit
        .column(Column::initial(115.0).at_least(95.0))  // 4. Fecha del commit
        .body(|mut body| {
            for entry in app.dir_entries.clone() {
                let icon = if entry.entry_type == "dir" { "📁" } else { "📄" };
                let is_opened = app.opened_file_path.as_deref() == Some(entry.path.as_str());

                body.row(24.0, |mut row| {
                    // Columna 1: Nombre archivo/carpeta
                    row.col(|ui| {
                        if ui.selectable_label(is_opened, format!("{} {}", icon, entry.name)).clicked() {
                            if entry.entry_type == "dir" {
                                app.current_path = entry.path;
                                app.load_directory(ctx.clone());
                            } else {
                                app.open_file(ctx.clone(), entry.path);
                            }
                        }
                    });

                    // Columna 2: Descripción del último commit (mismo tamaño estándar que en Repositorios)
                    row.col(|ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(&entry.last_commit_msg).weak())
                                .truncate(),
                        );
                    });

                    // Columna 3: Hash corto del commit (mismo tamaño estándar en monoespaciado)
                    row.col(|ui| {
                        ui.label(egui::RichText::new(&entry.last_commit_hash).monospace().weak());
                    });

                    // Columna 4: Fecha del commit (mismo tamaño estándar que en Repositorios)
                    row.col(|ui| {
                        ui.label(&entry.last_commit_date);
                    });
                });
            }
        });
}
