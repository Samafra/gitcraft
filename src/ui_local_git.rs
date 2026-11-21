//! Pantalla de Git local: lectura de estado, ramas, historial y diferencias.
//!
//! Esta vista sólo existe en escritorio y no depende de una sesión GitHub.
//! Los botones de E1 nunca ejecutan commit, checkout, pull, push ni reset:
//! delegan consultas al motor local de forma asíncrona para no congelar egui.

use crate::app::GitCraftApp;
use crate::local_git::LocalDiffTarget;
use eframe::egui;

/// Muestra el lector de repositorios locales y sus consultas disponibles.
///
/// app es el estado de la aplicación prestado de forma mutable: Rust permite
/// editarlo durante el dibujo y evita dos escrituras simultáneas.
/// ctx permite que el motor avise a egui cuando llega una respuesta.
pub fn render_local_git_tab(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    ui.heading(app.i18n.t(lang, "local_title"));
    ui.label(
        egui::RichText::new(app.i18n.t(lang, "local_readonly_note"))
            .weak()
            .small(),
    );
    ui.add_space(7.0);

    // La ruta es texto: se entrega como un argumento independiente a Git.
    // No se ejecuta un shell ni se guarda esta ruta en las preferencias.
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.local_repo_input)
                .desired_width(420.0)
                .hint_text(app.i18n.t(lang, "local_path_hint")),
        );
        if ui
            .add_enabled(
                !app.local_loading && !app.local_repo_input.trim().is_empty(),
                egui::Button::new(app.i18n.t(lang, "local_open")),
            )
            .clicked()
        {
            app.open_local_repository(ctx.clone());
        }
        if ui
            .add_enabled(
                !app.local_loading && app.local_repository.is_some(),
                egui::Button::new(app.i18n.t(lang, "local_refresh")),
            )
            .clicked()
        {
            app.refresh_local_repository(ctx.clone());
        }
    });

    // Las operaciones locales tienen su propio indicador; no afectan a GitHub/MCP.
    if app.local_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(app.i18n.t(lang, "local_loading"));
        });
    }
    if let Some(error) = &app.local_error {
        ui.colored_label(egui::Color32::LIGHT_RED, error);
    }

    // Option indica que quizá todavía no se haya abierto ningún repositorio.
    // Clonamos la fotografía inmutable para poder responder a clics sin
    // mantener prestado app durante la construcción de cada tabla.
    let Some(repository) = app.local_repository.clone() else {
        ui.add_space(15.0);
        ui.label(app.i18n.t(lang, "local_no_repository"));
        return;
    };

    ui.separator();
    ui.label(format!(
        "{}: {}",
        app.i18n.t(lang, "local_root"),
        repository.root.display()
    ));
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!(
            "{}: {}",
            app.i18n.t(lang, "local_head"),
            repository.head
        ));
        ui.separator();
        ui.label(&repository.git_version);
        ui.separator();
        ui.label(format!(
            "{}: {}",
            app.i18n.t(lang, "local_file_count"),
            repository.status.len()
        ));
        ui.label(format!(
            "{}: {}",
            app.i18n.t(lang, "local_branch_count"),
            repository.branches.len()
        ));
    });

    // La sección de estado muestra "XY" como lo devuelve Git:
    // X es el índice/staging y Y son los cambios sin preparar.
    ui.collapsing(
        format!(
            "{} ({})",
            app.i18n.t(lang, "local_status"),
            repository.status.len()
        ),
        |ui| {
            if repository.status.is_empty() {
                ui.label(app.i18n.t(lang, "local_clean"));
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("local_files_scroll")
                    .max_height(170.0)
                    .show(ui, |ui| {
                        egui::Grid::new("local_files_grid")
                            .striped(true)
                            .num_columns(2)
                            .show(ui, |ui| {
                                for file in &repository.status {
                                    ui.monospace(&file.code);
                                    let mut name = file.path.clone();
                                    if let Some(previous) = &file.original_path {
                                        name.push_str(" ← ");
                                        name.push_str(previous);
                                    }
                                    ui.label(name);
                                    ui.end_row();
                                }
                            });
                    });
            }
        },
    );

    ui.collapsing(
        format!(
            "{} ({})",
            app.i18n.t(lang, "local_branches"),
            repository.branches.len()
        ),
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt("local_branches_scroll")
                .max_height(170.0)
                .show(ui, |ui| {
                    for branch in &repository.branches {
                        // Las ramas remotas son refs guardadas localmente:
                        // no hacemos fetch ni consultamos servidores externos.
                        let source = if branch.remote {
                            app.i18n.t(lang, "local_remote")
                        } else {
                            app.i18n.t(lang, "local_branch")
                        };
                        ui.horizontal(|ui| {
                            ui.label(source);
                            ui.monospace(&branch.name);
                            if !branch.remote && branch.name == repository.head {
                                ui.strong("● HEAD");
                            }
                        });
                    }
                });
        },
    );

    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.heading(app.i18n.t(lang, "local_diff"));
        if ui
            .add_enabled(
                !app.local_diff_loading,
                egui::Button::new(app.i18n.t(lang, "local_diff_unstaged")),
            )
            .clicked()
        {
            app.load_local_diff(ctx.clone(), LocalDiffTarget::WorkingTree);
        }
        if ui
            .add_enabled(
                !app.local_diff_loading,
                egui::Button::new(app.i18n.t(lang, "local_diff_staged")),
            )
            .clicked()
        {
            app.load_local_diff(ctx.clone(), LocalDiffTarget::Staged);
        }
    });

    // El usuario puede consultar el diff de un commit pulsando su fila.
    // Nunca se hace checkout ni se cambia la rama para mostrar el historial.
    ui.collapsing(
        format!(
            "{} ({})",
            app.i18n.t(lang, "local_history"),
            repository.commits.len()
        ),
        |ui| {
            if repository.commits.is_empty() {
                ui.label(app.i18n.t(lang, "local_no_commits"));
            }
            egui::ScrollArea::vertical()
                .id_salt("local_history_scroll")
                .max_height(180.0)
                .show(ui, |ui| {
                    for commit in &repository.commits {
                        ui.horizontal(|ui| {
                            let selected = app.local_diff_target.as_ref()
                                == Some(&LocalDiffTarget::Commit(commit.sha.clone()));
                            if ui
                                .add_enabled(
                                    !app.local_diff_loading,
                                    egui::Button::new(
                                        egui::RichText::new(&commit.short_sha).monospace(),
                                    )
                                    .selected(selected),
                                )
                                .clicked()
                            {
                                app.load_local_diff(
                                    ctx.clone(),
                                    LocalDiffTarget::Commit(commit.sha.clone()),
                                );
                            }
                            ui.label(&commit.subject)
                                .on_hover_text(format!("{} · {}", commit.author, commit.date));
                        });
                    }
                });
        },
    );

    if app.local_diff_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(app.i18n.t(lang, "local_diff_loading"));
        });
    }

    if let Some(target) = &app.local_diff_target {
        let label = match target {
            LocalDiffTarget::WorkingTree => app.i18n.t(lang, "local_diff_unstaged").to_string(),
            LocalDiffTarget::Staged => app.i18n.t(lang, "local_diff_staged").to_string(),
            LocalDiffTarget::Commit(sha) => {
                format!("{} {}", app.i18n.t(lang, "local_commit"), sha)
            }
        };
        ui.label(egui::RichText::new(label).strong());
        if app.local_diff_text.is_empty() && !app.local_diff_loading {
            ui.label(app.i18n.t(lang, "local_empty_diff"));
        }
        // Un TextEdit no interactivo permite seleccionar y copiar el diff,
        // pero nunca editar el repositorio ni guardar esos cambios en disco.
        egui::ScrollArea::vertical()
            .id_salt("local_diff_scroll")
            .max_height(290.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut app.local_diff_text)
                        .code_editor()
                        .interactive(false)
                        .desired_width(f32::INFINITY)
                        .desired_rows(12),
                );
            });
    }

    ui.label(
        egui::RichText::new(app.i18n.t(lang, "local_untracked_note"))
            .weak()
            .small(),
    );
}
