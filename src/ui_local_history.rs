//! Interfaz E3-A: grafo de historial y herramientas Git avanzadas de lectura.
//!
//! La vista se dibuja dentro de la pestaña «Git local» de E1/E2, sin crear
//! operaciones de escritura. Todo dato pesado se consulta fuera del hilo
//! gráfico y la respuesta se identifica con un contador de generación.
//!
//! Los nombres seleccionados proceden exclusivamente de Git y no se
//! interpretan como comandos, expresiones de revisión ni rutas arbitrarias.

use crate::app::GitCraftApp;
use crate::local_git_history::{GitHistoryOverview, HistoryRevision};
use eframe::egui;

/// Petición de detalle que puede ejecutar E3-A sin modificar el repositorio.
#[derive(Debug, Clone)]
pub enum HistoryDetailRequest {
    /// Diff desde el commit izquierdo al derecho, ambos hashes completos.
    Comparison { left: String, right: String },
    /// Autoría por línea del archivo controlado por Git.
    Blame(String),
}

/// Estado temporal del panel avanzado, independiente de GitHub y MCP.
///
/// Default construye un estado sin datos ni selección. Todas las propiedades
/// se excluyen de las preferencias: no quedan rutas, diffs ni datos privados
/// del repositorio guardados en el fichero de configuración de GitCraft.
#[derive(Default)]
pub struct HistoryUiState {
    /// Grafo, etiquetas, stash y archivos seguidos obtenidos de Git.
    pub overview: Option<GitHistoryOverview>,
    /// Consulta general en curso.
    pub loading: bool,
    /// Error visible de la operación más reciente.
    pub error: Option<String>,
    /// SHA que actuará como extremo izquierdo de la comparación.
    pub base_sha: String,
    /// SHA que actuará como extremo derecho de la comparación.
    pub target_sha: String,
    /// Archivo seleccionado para consultar blame.
    pub selected_file: String,
    /// Indica si sigue consultándose un diff o blame.
    pub detail_loading: bool,
    /// Etiqueta descriptiva del último resultado.
    pub detail_label: String,
    /// Texto del último diff o blame.
    pub detail: String,
    /// Identificador de consulta general, para ignorar respuestas antiguas.
    pub overview_generation: u64,
    /// Identificador de consulta concreta, para ignorar respuestas antiguas.
    pub detail_generation: u64,
}

impl HistoryUiState {
    /// Descarta datos de otro repositorio, preservando los contadores.
    ///
    /// Rust no permite que dos respuestas antiguas suplanten una nueva:
    /// incrementar ambos números hace que sus mensajes sean rechazados
    /// cuando lleguen al canal asíncrono de la aplicación.
    pub fn invalidate(&mut self) {
        self.overview_generation = self.overview_generation.wrapping_add(1);
        self.detail_generation = self.detail_generation.wrapping_add(1);
        self.overview = None;
        self.loading = false;
        self.error = None;
        self.base_sha.clear();
        self.target_sha.clear();
        self.selected_file.clear();
        self.detail_loading = false;
        self.detail_label.clear();
        self.detail.clear();
    }

    /// Preselecciona dos hashes y el primer archivo sin ejecutar ninguna acción.
    ///
    /// Option representa una selección que puede estar vacía; por ejemplo,
    /// un repositorio recién inicializado todavía no tiene commits ni archivos.
    pub fn accept_overview(&mut self, overview: GitHistoryOverview) {
        self.base_sha = overview
            .revisions
            .get(1)
            .map(|revision| revision.sha.clone())
            .unwrap_or_default();
        self.target_sha = overview
            .revisions
            .first()
            .map(|revision| revision.sha.clone())
            .unwrap_or_default();
        self.selected_file = overview.tracked_files.first().cloned().unwrap_or_default();
        self.overview = Some(overview);
        self.loading = false;
        self.error = None;
        self.detail.clear();
        self.detail_label.clear();
        self.detail_generation = self.detail_generation.wrapping_add(1);
        self.detail_loading = false;
    }
}

/// Representa un commit en los menús sin mostrar necesariamente 40 caracteres.
fn revision_label(revisions: &[HistoryRevision], sha: &str) -> String {
    revisions
        .iter()
        .find(|revision| revision.sha == sha)
        .map(|revision| format!("{}  {}", revision.short_sha, revision.subject))
        .unwrap_or_else(|| "—".to_string())
}

/// Muestra el panel avanzado, que nunca pide autenticación GitHub.
///
/// La interfaz utiliza valores de estado de GitCraft que viven sólo en RAM.
pub fn render_history_panel(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    ui.separator();
    ui.collapsing(app.i18n.t(lang, "e3_title"), |ui| {
        ui.label(
            egui::RichText::new(app.i18n.t(lang, "e3_readonly_note"))
                .weak()
                .small(),
        );

        let matches_root = app
            .local_repository
            .as_ref()
            .map(|repo| app.local_repo_input.as_str() == repo.root.to_string_lossy().as_ref())
            .unwrap_or(false);
        let can_query = matches_root && !app.local_loading && !app.local_action_loading;

        if ui
            .add_enabled(
                can_query && !app.history.loading,
                egui::Button::new(app.i18n.t(lang, "e3_load")),
            )
            .clicked()
        {
            app.load_advanced_history(ctx.clone());
        }
        if app.history.loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(app.i18n.t(lang, "e3_loading"));
            });
        }
        if let Some(error) = &app.history.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }

        // Sólo se copia la fotografía del grafo; las consultas Git se realizan
        // fuera de egui. Los botones pueden modificar app sin préstamos vivos.
        let Some(overview) = app.history.overview.clone() else {
            ui.label(app.i18n.t(lang, "e3_empty"));
            return;
        };
        if !matches_root
            || app.local_repository.as_ref().map(|repo| &repo.root) != Some(&overview.root)
        {
            ui.label(app.i18n.t(lang, "e3_reload_required"));
            return;
        }

        ui.collapsing(app.i18n.t(lang, "e3_graph"), |ui| {
            if overview.graph.is_empty() {
                ui.label(app.i18n.t(lang, "e3_no_commits"));
            } else {
                let mut graph = overview.graph.clone();
                egui::ScrollArea::vertical()
                    .id_salt("e3_graph_scroll")
                    .max_height(250.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut graph)
                                .code_editor()
                                .interactive(false)
                                .desired_width(f32::INFINITY)
                                .desired_rows(10),
                        );
                    });
            }
        });

        ui.group(|ui| {
            ui.label(egui::RichText::new(app.i18n.t(lang, "e3_compare")).strong());
            if overview.revisions.len() < 2 {
                ui.label(app.i18n.t(lang, "e3_need_two_commits"));
                return;
            }

            // Los dos extremos sólo se eligen entre commits obtenidos de Git.
            ui.horizontal_wrapped(|ui| {
                ui.label(app.i18n.t(lang, "e3_base"));
                egui::ComboBox::from_id_salt("e3_compare_base")
                    .selected_text(revision_label(
                        &overview.revisions,
                        &app.history.base_sha,
                    ))
                    .width(250.0)
                    .show_ui(ui, |ui| {
                        for revision in &overview.revisions {
                            ui.selectable_value(
                                &mut app.history.base_sha,
                                revision.sha.clone(),
                                revision_label(&overview.revisions, &revision.sha),
                            );
                        }
                    });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(app.i18n.t(lang, "e3_target"));
                egui::ComboBox::from_id_salt("e3_compare_target")
                    .selected_text(revision_label(
                        &overview.revisions,
                        &app.history.target_sha,
                    ))
                    .width(250.0)
                    .show_ui(ui, |ui| {
                        for revision in &overview.revisions {
                            ui.selectable_value(
                                &mut app.history.target_sha,
                                revision.sha.clone(),
                                revision_label(&overview.revisions, &revision.sha),
                            );
                        }
                    });
            });
            if ui
                .add_enabled(
                    can_query
                        && !app.history.detail_loading
                        && app.history.base_sha != app.history.target_sha,
                    egui::Button::new(app.i18n.t(lang, "e3_compare_button")),
                )
                .clicked()
            {
                app.load_advanced_detail(
                    ctx.clone(),
                    HistoryDetailRequest::Comparison {
                        left: app.history.base_sha.clone(),
                        right: app.history.target_sha.clone(),
                    },
                );
            }
        });

        ui.group(|ui| {
            ui.label(egui::RichText::new(app.i18n.t(lang, "e3_blame")).strong());
            if overview.tracked_files.is_empty() {
                ui.label(app.i18n.t(lang, "e3_no_tracked_files"));
            } else {
                egui::ComboBox::from_id_salt("e3_blame_file")
                    .selected_text(&app.history.selected_file)
                    .width(320.0)
                    .show_ui(ui, |ui| {
                        for path in &overview.tracked_files {
                            ui.selectable_value(
                                &mut app.history.selected_file,
                                path.clone(),
                                path,
                            );
                        }
                    });
                if ui
                    .add_enabled(
                        can_query && !app.history.detail_loading,
                        egui::Button::new(app.i18n.t(lang, "e3_blame_button")),
                    )
                    .clicked()
                {
                    app.load_advanced_detail(
                        ctx.clone(),
                        HistoryDetailRequest::Blame(app.history.selected_file.clone()),
                    );
                }
                if overview.tracked_truncated {
                    ui.label(app.i18n.t(lang, "e3_tracked_limit"));
                }
            }
        });

        // Estas listas son información de recuperación. No muestran
        // controles para aplicar stash o reescribir referencias.
        ui.collapsing(
            format!("{} ({})", app.i18n.t(lang, "e3_reflog"), overview.reflog.len()),
            |ui| {
                draw_lines(ui, "e3_reflog_scroll", &overview.reflog);
            },
        );
        ui.collapsing(
            format!("{} ({})", app.i18n.t(lang, "e3_tags"), overview.tags.len()),
            |ui| {
                draw_lines(ui, "e3_tags_scroll", &overview.tags);
            },
        );
        ui.collapsing(
            format!("{} ({})", app.i18n.t(lang, "e3_stash"), overview.stash.len()),
            |ui| {
                draw_lines(ui, "e3_stash_scroll", &overview.stash);
            },
        );

        if app.history.detail_loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(app.i18n.t(lang, "e3_detail_loading"));
            });
        }
        if !app.history.detail_label.is_empty() {
            ui.label(egui::RichText::new(&app.history.detail_label).strong());
            if app.history.detail.is_empty() && !app.history.detail_loading {
                ui.label(app.i18n.t(lang, "e3_no_diff"));
            }
            let mut result = app.history.detail.clone();
            egui::ScrollArea::vertical()
                .id_salt("e3_detail_scroll")
                .max_height(350.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut result)
                            .code_editor()
                            .interactive(false)
                            .desired_width(f32::INFINITY)
                            .desired_rows(12),
                    );
                });
        }
    });
}

/// Dibuja líneas de una lista Git sin permitir ejecutar ninguna de ellas.
fn draw_lines(ui: &mut egui::Ui, id: &str, entries: &[String]) {
    if entries.is_empty() {
        ui.label("—");
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt(id)
        .max_height(140.0)
        .show(ui, |ui| {
            for entry in entries {
                ui.monospace(entry);
            }
        });
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Construye un resumen ficticio sin ejecutar Git ni acceder a archivos.
    /// Así se comprueba el estado de la UI independientemente del motor.
    fn fixture_overview() -> GitHistoryOverview {
        GitHistoryOverview {
            root: PathBuf::from("/gitcraft-e3-test-ficticio"),
            graph: "* abc123 Primer commit".into(),
            revisions: vec![
                HistoryRevision {
                    sha: "a".repeat(40),
                    short_sha: "aaaaaaa".into(),
                    subject: "Segundo".into(),
                },
                HistoryRevision {
                    sha: "b".repeat(40),
                    short_sha: "bbbbbbb".into(),
                    subject: "Primero".into(),
                },
            ],
            reflog: vec!["reflog ficticio".into()],
            tags: vec!["test-v1".into()],
            stash: Vec::new(),
            tracked_files: vec!["archivo.txt".into()],
            tracked_truncated: false,
        }
    }

    /// La selección inicial propone dos revisiones diferentes y un archivo.
    #[test]
    fn history_defaults_select_two_distinct_commits() {
        let mut state = HistoryUiState::default();
        state.accept_overview(fixture_overview());
        assert_eq!(state.base_sha, "b".repeat(40));
        assert_eq!(state.target_sha, "a".repeat(40));
        assert_eq!(state.selected_file, "archivo.txt");
        assert!(!state.loading);
    }

    /// Cambiar de repositorio revoca la consulta y descarta datos previos.
    #[test]
    fn invalidate_discards_data_and_advances_generations() {
        let mut state = HistoryUiState::default();
        state.accept_overview(fixture_overview());
        state.detail = "un diff antiguo".into();
        state.detail_label = "Blame viejo".into();
        let previous_overview = state.overview_generation;
        let previous_detail = state.detail_generation;

        state.invalidate();

        assert!(state.overview.is_none());
        assert!(state.detail.is_empty());
        assert!(state.detail_label.is_empty());
        assert!(state.base_sha.is_empty());
        assert!(state.selected_file.is_empty());
        assert_ne!(state.overview_generation, previous_overview);
        assert_ne!(state.detail_generation, previous_detail);
    }
}
