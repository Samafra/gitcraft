//! Vista de archivos locales reorganizada en dos columnas, al estilo SourceTree.
//!
//! A la izquierda se separan «Sin preparar» y «Preparados» para que el usuario
//! vea de inmediato qué entrará en el commit. A la derecha se muestra el diff
//! del archivo elegido. Abajo hay un único panel de mensaje y confirmación.
//!
//! Todos los controles siguen utilizando el motor E2 validado: el primer clic
//! en Preparar, Retirar o Commit crea sólo una vista previa. El segundo clic
//! confirma tras volver a comprobar el repositorio. No se ejecuta checkout,
//! reset, clean, push ni ningún comando nuevo desde esta pantalla.

use crate::app::GitCraftApp;
use crate::local_git::{LocalDiffTarget, LocalFileStatus};
use crate::local_git_actions::LocalAction;
use eframe::egui;

/// Muestra únicamente el estado de archivos, diffs y commits E2.
///
/// La navegación, la ruta del repositorio y las ramas viven ahora en el
/// shell lateral ui_shell, así que no se repiten aquí.
pub fn render_local_git_tab(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let Some(repository) = app.local_repository.clone() else {
        ui.label(app.i18n.t(app.language, "local_no_repository"));
        return;
    };
    let lang = app.language;
    let path_matches =
        app.local_repo_input.as_str() == repository.root.to_string_lossy().as_ref();
    let can_write = path_matches
        && !app.local_loading
        && !app.local_action_loading
        && !app.history.ref_loading;

    ui.horizontal_wrapped(|ui| {
        ui.heading(app.i18n.t(lang, "ux_file_status"));
        ui.separator();
        ui.strong(format!(
            "{} {}",
            repository.status.len(),
            app.i18n.t(lang, "ux_pending_short")
        ));
    });
    ui.add_space(5.0);

    // La misma fotografía de Git se usa en ambas columnas. Los callbacks
    // únicamente solicitan tareas; nunca modifican los archivos directamente.
    ui.columns(2, |columns| {
        let file_column = &mut columns[0];
        file_column.heading(app.i18n.t(lang, "ux_working_copy"));
        file_column.separator();
        let (unstaged, staged) = count_categories(&repository.status);
        egui::ScrollArea::vertical()
            .id_salt("gc_status_file_scroll")
            .max_height(410.0)
            .show(file_column, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} ({})",
                        app.i18n.t(lang, "ux_unstaged_files"),
                        unstaged
                    ))
                    .strong(),
                );
                if unstaged == 0 {
                    ui.label(app.i18n.t(lang, "ux_nothing_unstaged"));
                } else {
                    for file in &repository.status {
                        let (file_unstaged, _) = categories(&file.code);
                        if file_unstaged {
                            render_file_row(app, ui, ctx, file, false, can_write);
                        }
                    }
                }
                ui.add_space(12.0);
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "{} ({})",
                        app.i18n.t(lang, "ux_staged_files"),
                        staged
                    ))
                    .strong(),
                );
                if staged == 0 {
                    ui.label(app.i18n.t(lang, "ux_nothing_staged"));
                } else {
                    for file in &repository.status {
                        let (_, file_staged) = categories(&file.code);
                        if file_staged {
                            render_file_row(app, ui, ctx, file, true, can_write);
                        }
                    }
                }
            });

        let diff_column = &mut columns[1];
        diff_column.heading(app.i18n.t(lang, "ux_diff_preview"));
        diff_column.separator();
        render_diff_column(app, diff_column, ctx);
    });

    ui.add_space(9.0);
    ui.separator();
    render_commit_panel(app, ui, ctx, can_write);
}

/// Identifica cambios sin preparar y preparados a partir del código XY de Git.
///
/// Git escribe dos caracteres: el primero refleja el índice; el segundo,
/// el directorio de trabajo. «??» representa un archivo nuevo no seguido.
/// Una entrada puede aparecer en ambas listas si tiene cambios de los dos tipos.
fn categories(code: &str) -> (bool, bool) {
    if code == "??" {
        return (true, false);
    }
    let bytes = code.as_bytes();
    if bytes.len() != 2 {
        return (false, false);
    }
    (bytes[1] != b' ', bytes[0] != b' ')
}

/// Cuenta ambas categorías sin modificar la lista de archivos Git.
fn count_categories(files: &[LocalFileStatus]) -> (usize, usize) {
    let mut unstaged = 0;
    let mut staged = 0;
    for file in files {
        let (has_unstaged, has_staged) = categories(&file.code);
        if has_unstaged {
            unstaged += 1;
        }
        if has_staged {
            staged += 1;
        }
    }
    (unstaged, staged)
}

/// Muestra un archivo y la acción E2 adecuada a su categoría.
///
/// Pulsar el nombre sólo consulta sus diferencias. Preparar y Retirar
/// muestran la confirmación protegida del motor E2, nunca escriben al instante.
fn render_file_row(
    app: &mut GitCraftApp,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    file: &LocalFileStatus,
    staged: bool,
    can_write: bool,
) {
    let lang = app.language;
    let target = LocalDiffTarget::File {
        path: file.path.clone(),
        staged,
    };
    let selected = app.local_diff_target.as_ref() == Some(&target);

    ui.horizontal_wrapped(|ui| {
        ui.monospace(&file.code);
        let label = if let Some(previous) = &file.original_path {
            format!("{} ← {}", file.path, previous)
        } else {
            file.path.clone()
        };
        if ui
            .selectable_label(selected, &label)
            .on_hover_text(&label)
            .clicked()
        {
            app.load_local_diff(ctx.clone(), target);
        }

        let key = if staged { "e2_unstage" } else { "e2_stage" };
        if ui
            .add_enabled(
                can_write,
                egui::Button::new(app.i18n.t(lang, key)).small(),
            )
            .clicked()
        {
            let action = if staged {
                LocalAction::Unstage(file.path.clone())
            } else {
                LocalAction::Stage(file.path.clone())
            };
            app.preview_local_action(ctx.clone(), action);
        }
    });
}

/// Columna de diferencias con selección de archivos y botones generales.
///
/// Las consultas de índice y directorio de trabajo reutilizan E1, no escriben.
/// El texto es copiable pero no editable; cambiarlo no afecta al repositorio.
fn render_diff_column(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    ui.horizontal_wrapped(|ui| {
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
    if app.local_diff_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(app.i18n.t(lang, "local_diff_loading"));
        });
    }
    if let Some(target) = &app.local_diff_target {
        let label = match target {
            LocalDiffTarget::WorkingTree => {
                app.i18n.t(lang, "local_diff_unstaged").to_string()
            }
            LocalDiffTarget::Staged => app.i18n.t(lang, "local_diff_staged").to_string(),
            LocalDiffTarget::Commit(sha) => format!(
                "{} {}",
                app.i18n.t(lang, "local_commit"),
                sha
            ),
            LocalDiffTarget::File { path, staged } => format!(
                "{} ({})",
                path,
                if *staged {
                    app.i18n.t(lang, "e2_diff_index")
                } else {
                    app.i18n.t(lang, "local_diff_unstaged")
                }
            ),
        };
        ui.label(egui::RichText::new(label).strong());
    } else {
        ui.label(app.i18n.t(lang, "ux_select_file_hint"));
    }
    if app.local_diff_target.is_some()
        && app.local_diff_text.is_empty()
        && !app.local_diff_loading
    {
        ui.label(app.i18n.t(lang, "local_empty_diff"));
    }
    egui::ScrollArea::vertical()
        .id_salt("gc_selected_diff_scroll")
        .max_height(360.0)
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut app.local_diff_text)
                    .code_editor()
                    .interactive(false)
                    .desired_width(f32::INFINITY)
                    .desired_rows(16),
            );
        });
}

/// Área inferior de commit y de confirmación para toda operación E2.
///
/// La validación del motor sigue rechazando cambios en archivos, HEAD y
/// estado ocurridos después de previsualizar, igual que en E2 validada.
fn render_commit_panel(
    app: &mut GitCraftApp,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    can_write: bool,
) {
    let lang = app.language;
    ui.group(|ui| {
        ui.label(egui::RichText::new(app.i18n.t(lang, "ux_commit_message")).strong());
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled(
                !app.local_action_loading,
                egui::TextEdit::singleline(&mut app.local_commit_message)
                    .desired_width(480.0)
                    .hint_text(app.i18n.t(lang, "e2_commit_hint")),
            );
            let can_preview = can_write && !app.local_commit_message.trim().is_empty();
            if ui
                .add_enabled(
                    can_preview,
                    egui::Button::new(app.i18n.t(lang, "e2_preview_commit")),
                )
                .clicked()
            {
                app.preview_local_action(
                    ctx.clone(),
                    LocalAction::Commit(app.local_commit_message.trim().to_string()),
                );
            }
        });
        ui.label(
            egui::RichText::new(app.i18n.t(lang, "e2_commit_warning"))
                .weak()
                .small(),
        );
    });

    if app.local_action_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(app.i18n.t(lang, "e2_action_loading"));
        });
    }

    // El resumen sólo existe si el motor E2 autorizó la primera etapa.
    // No se clona la huella completa del índice para dibujar la interfaz.
    if let Some(preview) = app.local_action_preview.as_ref() {
        let summary = preview.summary.clone();
        let details = preview.details.clone();
        let action = preview.action.clone();
        ui.group(|ui| {
            ui.label(egui::RichText::new(app.i18n.t(lang, "e2_confirm_title")).strong());
            ui.label(summary);
            egui::ScrollArea::vertical()
                .id_salt("gc_commit_preview_scroll")
                .max_height(130.0)
                .show(ui, |ui| {
                    ui.monospace(&details);
                });
            ui.label(
                egui::RichText::new(app.i18n.t(lang, "e2_confirm_notice"))
                    .weak()
                    .small(),
            );
            ui.horizontal(|ui| {
                let selection_unchanged = match &action {
                    LocalAction::Commit(message) => {
                        message.as_str() == app.local_commit_message.trim()
                    }
                    _ => true,
                };
                if ui
                    .add_enabled(
                        selection_unchanged && can_write,
                        egui::Button::new(app.i18n.t(lang, "e2_confirm")),
                    )
                    .clicked()
                {
                    app.confirm_local_action(ctx.clone());
                }
                if ui.button(app.i18n.t(lang, "e2_cancel")).clicked() {
                    app.cancel_local_action();
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El estado de Git identifica archivos nuevos como pendientes.
    #[test]
    fn untracked_file_is_only_unstaged() {
        assert_eq!(categories("??"), (true, false));
    }

    /// Los cambios preparados y sin preparar se muestran por separado.
    #[test]
    fn staged_and_unstaged_categories_are_distinct() {
        assert_eq!(categories(" M"), (true, false));
        assert_eq!(categories("M "), (false, true));
        assert_eq!(categories("MM"), (true, true));
        assert_eq!(categories("D "), (false, true));
        assert_eq!(categories("  "), (false, false));
    }
}
