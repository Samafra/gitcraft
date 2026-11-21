//! Interfaz de GitCraft inspirada en la distribución de SourceTree.
//!
//! Este módulo sustituye la fila de ocho pestañas por una navegación lateral
//! persistente: ESPACIO DE TRABAJO, REFERENCIAS, GITHUB y HERRAMIENTAS.
//! La barra superior contiene acciones frecuentes; la vista central cambia
//! según el destino seleccionado. El motor Git y sus confirmaciones E2/E3-B
//! permanecen en sus módulos originales, sin introducir escrituras nuevas.
//!
//! Las funciones de Git local sólo están disponibles en escritorio. En el
//! navegador se mantiene la conexión GitHub y las páginas remotas existentes.

use crate::app::{ActiveTab, GitCraftApp};
use crate::i18n::Language;
#[cfg(not(target_arch = "wasm32"))]
use crate::local_git_refs::RefKind;
#[cfg(not(target_arch = "wasm32"))]
use crate::ui_local_git::render_local_git_tab;
#[cfg(not(target_arch = "wasm32"))]
use crate::ui_local_history::{render_history_panel, render_ref_creation_panel};
use crate::ui_changes::render_changes_tab;
use crate::ui_manage::{
    render_actions_tab, render_issues_tab, render_prs_tab, render_releases_tab,
};
use crate::ui_mcp::render_mcp_ui;
use crate::ui_repos::render_repos_tab;
use eframe::egui;

/// Pantalla del espacio de trabajo Git local, independiente del acceso a GitHub.
///
/// Estos valores sólo cambian la presentación; no representan comandos Git.
/// En concreto, visitar Ramas nunca hace checkout y visitar Stash nunca
/// aplica ni elimina una entrada guardada.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WorkspacePane {
    /// Archivo modificado, índice y creación confirmada de commits.
    #[default]
    FileStatus,
    /// Grafo, selección de commits, comparación y blame.
    History,
    /// Ramas locales y creación de referencias, sin checkout.
    Branches,
    /// Etiquetas locales y creación de etiquetas ligeras.
    Tags,
    /// Ramas remotas conocidas localmente, sin consulta de red.
    Remotes,
    /// Entradas stash guardadas, exclusivamente de lectura.
    Stashes,
}

/// Selecciona una pantalla sin ejecutar operaciones sobre el repositorio.
///
/// La primera apertura del historial puede iniciar una consulta asíncrona
/// de solo lectura. Si otra consulta ya está en curso, se reutilizará.
#[cfg(not(target_arch = "wasm32"))]
fn select_workspace(app: &mut GitCraftApp, pane: WorkspacePane, ctx: &egui::Context) {
    app.active_tab = ActiveTab::LocalGit;
    app.workspace_pane = pane;
    if pane != WorkspacePane::FileStatus
        && app.history.overview.is_none()
        && app.local_repository.is_some()
    {
        app.load_advanced_history(ctx.clone());
    }
}

/// Dibuja toda la ventana con cabecera, menú lateral y contenido principal.
///
/// La barra superior y el menú permanecen en el mismo sitio aunque se cambie
/// de pantalla; ya no hay que buscar botones dentro de una página muy larga.
pub fn render_app_shell(app: &mut GitCraftApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("gc_top_toolbar")
        .resizable(false)
        .show(ctx, |ui| {
            render_header(app, ui);
            #[cfg(not(target_arch = "wasm32"))]
            if app.active_tab == ActiveTab::LocalGit {
                ui.separator();
                render_local_toolbar(app, ui, ctx);
            }
        });

    egui::TopBottomPanel::bottom("gc_status_bar")
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                #[cfg(not(target_arch = "wasm32"))]
                if app.active_tab == ActiveTab::LocalGit {
                    if let Some(repo) = &app.local_repository {
                        ui.strong(format!("● {}", repo.head));
                        ui.separator();
                        ui.label(format!(
                            "{} {}",
                            repo.status.len(),
                            app.i18n.t(app.language, "ux_pending_short")
                        ));
                    } else {
                        ui.label(app.i18n.t(app.language, "ux_no_repo"));
                    }
                }
                if let Some((notice, is_error)) = &app.status_notice {
                    ui.colored_label(
                        if *is_error {
                            egui::Color32::from_rgb(210, 61, 72)
                        } else {
                            egui::Color32::from_rgb(36, 134, 88)
                        },
                        notice,
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(user) = &app.user {
                        ui.label(format!("@{}", user.login));
                    }
                });
            });
        });

    egui::SidePanel::left("gc_repository_sidebar")
        .default_width(238.0)
        .min_width(185.0)
        .max_width(310.0)
        .resizable(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("gc_sidebar_scroll")
                .show(ui, |ui| render_sidebar(app, ui, ctx));
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        #[cfg(not(target_arch = "wasm32"))]
        if app.active_tab == ActiveTab::LocalGit {
            render_local_workspace(app, ui, ctx);
            return;
        }

        // No se requiere PAT para Git local. Las páginas de GitHub sí deben
        // solicitar autorización antes de mostrar operaciones remotas.
        if app.user.is_none() {
            render_github_connection(app, ui, ctx);
            return;
        }

        match app.active_tab {
            ActiveTab::Repositories => render_repos_tab(app, ui, ctx),
            ActiveTab::Changes => render_changes_tab(app, ui, ctx),
            ActiveTab::PullRequests => render_prs_tab(app, ui, ctx),
            ActiveTab::Issues => render_issues_tab(app, ui, ctx),
            ActiveTab::Actions => render_actions_tab(app, ui, ctx),
            ActiveTab::Releases => render_releases_tab(app, ui, ctx),
            ActiveTab::Mcp => render_mcp_ui(app, ui, ctx),
            ActiveTab::LocalGit => {
                ui.label(app.i18n.t(app.language, "local_web_unavailable"));
            }
        }
    });
}

/// Cabecera sobria: nombre del repositorio, modo de trabajo y preferencias.
///
/// La selección de idioma y contraste está siempre en el mismo extremo;
/// los botones de Git local se muestran en una línea separada.
fn render_header(app: &mut GitCraftApp, ui: &mut egui::Ui) {
    let lang = app.language;
    ui.horizontal(|ui| {
        ui.heading("GitCraft");
        ui.separator();
        #[cfg(not(target_arch = "wasm32"))]
        if app.active_tab == ActiveTab::LocalGit {
            if let Some(repo) = &app.local_repository {
                let repo_name = repo
                    .root
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| repo.root.display().to_string());
                ui.strong(repo_name);
                ui.label(format!("· {}", repo.head));
            } else {
                ui.label(app.i18n.t(lang, "ux_no_repo"));
            }
        }
        if app.active_tab != ActiveTab::LocalGit {
            ui.label(app.i18n.t(lang, "ux_github_header"));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(if app.dark_mode { "☀" } else { "☾" })
                .on_hover_text(app.i18n.t(lang, "ux_theme"))
                .clicked()
            {
                app.dark_mode = !app.dark_mode;
                app.apply_theme(ui.ctx());
            }
            egui::ComboBox::from_id_salt("gc_lang")
                .selected_text(lang.label())
                .show_ui(ui, |ui| {
                    for language in Language::all() {
                        ui.selectable_value(&mut app.language, *language, language.label());
                    }
                });
        });
    });
}

/// Barra de acciones frecuentes similar a la organización de SourceTree.
///
/// Sólo se habilitan las operaciones ya implementadas y verificadas.
#[cfg(not(target_arch = "wasm32"))]
fn render_local_toolbar(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    let available = app.local_repository.is_some()
        && !app.local_loading
        && !app.local_action_loading
        && !app.history.ref_loading;
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                available,
                egui::Button::new(app.i18n.t(lang, "ux_commit")),
            )
            .clicked()
        {
            select_workspace(app, WorkspacePane::FileStatus, ctx);
        }
        if ui
            .add_enabled(
                available,
                egui::Button::new(app.i18n.t(lang, "ux_history")),
            )
            .clicked()
        {
            select_workspace(app, WorkspacePane::History, ctx);
        }
        if ui
            .add_enabled(
                available,
                egui::Button::new(app.i18n.t(lang, "ux_branch")),
            )
            .clicked()
        {
            select_workspace(app, WorkspacePane::Branches, ctx);
            app.history.ref_kind = RefKind::Branch;
            app.cancel_local_ref();
        }
        if ui
            .add_enabled(
                available,
                egui::Button::new(app.i18n.t(lang, "ux_tag")),
            )
            .clicked()
        {
            select_workspace(app, WorkspacePane::Tags, ctx);
            app.history.ref_kind = RefKind::Tag;
            app.cancel_local_ref();
        }
        ui.separator();
        if ui
            .add_enabled(
                available,
                egui::Button::new(app.i18n.t(lang, "ux_refresh")),
            )
            .clicked()
        {
            app.refresh_local_repository(ctx.clone());
        }

        // La similitud visual no justifica habilitar comandos que aún no
        // disponen de confirmación y pruebas. No tienen acción oculta.
        for key in ["ux_fetch", "ux_pull", "ux_push", "ux_merge", "ux_stash_action"] {
            ui.add_enabled(false, egui::Button::new(app.i18n.t(lang, key)))
                .on_hover_text(app.i18n.t(lang, "ux_unavailable"));
        }
    });
}

/// Menú lateral: navegación con nombres y categorías siempre visibles.
///
/// Los grupos de ramas, etiquetas y stash muestran los valores disponibles
/// localmente, pero nunca ejecutan operaciones al seleccionar un nombre.
fn render_sidebar(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(app.i18n.t(lang, "ux_workspace"))
            .strong()
            .small(),
    );
    ui.add_space(4.0);

    #[cfg(not(target_arch = "wasm32"))]
    {
        sidebar_local_item(app, ui, ctx, WorkspacePane::FileStatus, "ux_file_status");
        sidebar_local_item(app, ui, ctx, WorkspacePane::History, "ux_history");
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(app.i18n.t(lang, "ux_references"))
                .strong()
                .small(),
        );
        sidebar_local_item(app, ui, ctx, WorkspacePane::Branches, "ux_branches");
        sidebar_local_item(app, ui, ctx, WorkspacePane::Tags, "ux_tags");
        sidebar_local_item(app, ui, ctx, WorkspacePane::Remotes, "ux_remotes");
        sidebar_local_item(app, ui, ctx, WorkspacePane::Stashes, "ux_stashes");

        // Cada nombre representa un enlace de navegación. La rama visible
        // nunca se cambia mediante un clic en esta barra lateral.
        if let Some(repo) = &app.local_repository {
            let local: Vec<String> = repo
                .branches
                .iter()
                .filter(|branch| !branch.remote)
                .take(12)
                .map(|branch| branch.name.clone())
                .collect();
            let current = repo.head.clone();
            if !local.is_empty() {
                let group_title = app.i18n.t(lang, "ux_local_branches").to_owned();
                egui::CollapsingHeader::new(group_title)
                    .default_open(true)
                    .show(ui, |ui| {
                        for name in &local {
                            let marker = if *name == current { "● " } else { "   " };
                            if ui
                                .selectable_label(
                                    false,
                                    format!("{}{}", marker, name),
                                )
                                .on_hover_text(app.i18n.t(lang, "ux_no_checkout"))
                                .clicked()
                            {
                                select_workspace(app, WorkspacePane::Branches, ctx);
                            }
                        }
                    });
            }
        }
    }

    ui.add_space(14.0);
    ui.separator();
    ui.label(
        egui::RichText::new(app.i18n.t(lang, "ux_online"))
            .strong()
            .small(),
    );
    sidebar_remote_item(app, ui, ActiveTab::Repositories, "tab_repos");
    sidebar_remote_item(app, ui, ActiveTab::Changes, "tab_changes");
    sidebar_remote_item(app, ui, ActiveTab::PullRequests, "tab_prs");
    sidebar_remote_item(app, ui, ActiveTab::Issues, "tab_issues");
    sidebar_remote_item(app, ui, ActiveTab::Actions, "tab_actions");
    sidebar_remote_item(app, ui, ActiveTab::Releases, "tab_releases");
    ui.add_space(8.0);
    ui.separator();
    sidebar_remote_item(app, ui, ActiveTab::Mcp, "tab_mcp");
}

/// Botón lateral para una subpantalla Git nativa.
///
/// Sólo altera la vista seleccionada; la carga inicial del historial es
/// exclusivamente de lectura y se ejecuta en una tarea independiente.
#[cfg(not(target_arch = "wasm32"))]
fn sidebar_local_item(
    app: &mut GitCraftApp,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    pane: WorkspacePane,
    key: &str,
) {
    let label = app.i18n.t(app.language, key).to_owned();
    let selected = app.active_tab == ActiveTab::LocalGit && app.workspace_pane == pane;
    if ui.selectable_label(selected, label).clicked() {
        select_workspace(app, pane, ctx);
    }
}

/// Botón lateral para las funciones GitHub y MCP que ya existían.
///
/// Se conserva su propia lógica de autenticación y sus acciones remotas.
/// Cambiar de página no transmite tokens ni inicia peticiones por sí mismo.
fn sidebar_remote_item(app: &mut GitCraftApp, ui: &mut egui::Ui, tab: ActiveTab, key: &str) {
    let label = app.i18n.t(app.language, key).to_owned();
    if ui
        .selectable_label(app.active_tab == tab, label)
        .clicked()
    {
        app.active_tab = tab;
    }
}

/// Área central de Git local con ruta de carpeta siempre visible arriba.
///
/// Las rutas siguen verificándose con la lógica E1/E2/E3-B. El contenido
/// se distribuye entre la vista de archivos, el historial y las referencias.
#[cfg(not(target_arch = "wasm32"))]
fn render_local_workspace(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    ui.horizontal_wrapped(|ui| {
        ui.label(app.i18n.t(lang, "ux_repository"));
        let changed = ui
            .add_enabled(
                !app.local_action_loading && !app.history.ref_loading,
                egui::TextEdit::singleline(&mut app.local_repo_input)
                    .desired_width(430.0)
                    .hint_text(app.i18n.t(lang, "local_path_hint")),
            )
            .changed();
        if changed {
            app.cancel_local_action();
            app.cancel_local_ref();
        }
        let may_open = !app.local_loading
            && !app.local_action_loading
            && !app.history.ref_loading
            && !app.local_repo_input.trim().is_empty();
        if ui
            .add_enabled(
                may_open,
                egui::Button::new(app.i18n.t(lang, "local_open")),
            )
            .clicked()
        {
            app.workspace_pane = WorkspacePane::FileStatus;
            app.open_local_repository(ctx.clone());
        }
    });
    if let Some(error) = &app.local_error {
        ui.colored_label(egui::Color32::from_rgb(205, 55, 66), error);
    }
    if let Some(notice) = &app.local_action_notice {
        ui.colored_label(egui::Color32::from_rgb(37, 131, 82), notice);
    }
    if app.local_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(app.i18n.t(lang, "local_loading"));
        });
    }

    ui.add_space(6.0);
    ui.separator();

    let Some(repo) = &app.local_repository else {
        ui.add_space(25.0);
        ui.heading(app.i18n.t(lang, "ux_open_title"));
        ui.label(app.i18n.t(lang, "ux_open_hint"));
        return;
    };
    if app.local_repo_input.as_str() != repo.root.to_string_lossy().as_ref() {
        ui.colored_label(
            egui::Color32::from_rgb(195, 116, 33),
            app.i18n.t(lang, "ux_path_changed"),
        );
        return;
    }

    match app.workspace_pane {
        WorkspacePane::FileStatus => render_local_git_tab(app, ui, ctx),
        WorkspacePane::History => {
            ui.heading(app.i18n.t(lang, "ux_history"));
            egui::ScrollArea::vertical()
                .id_salt("gc_history_main")
                .show(ui, |ui| render_history_panel(app, ui, ctx));
        }
        WorkspacePane::Branches => {
            render_references_page(app, ui, ctx, WorkspacePane::Branches);
        }
        WorkspacePane::Tags => {
            render_references_page(app, ui, ctx, WorkspacePane::Tags);
        }
        WorkspacePane::Remotes => {
            render_references_page(app, ui, ctx, WorkspacePane::Remotes);
        }
        WorkspacePane::Stashes => {
            render_references_page(app, ui, ctx, WorkspacePane::Stashes);
        }
    }
}

/// Vista de ramas, etiquetas, remotos o stash separada del historial.
///
/// Las listas son de consulta; el formulario E3-B sólo aparece en Ramas
/// y Etiquetas, y conserva la doble confirmación existente.
#[cfg(not(target_arch = "wasm32"))]
fn render_references_page(
    app: &mut GitCraftApp,
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    pane: WorkspacePane,
) {
    let lang = app.language;
    let title_key = match pane {
        WorkspacePane::Branches => "ux_branches",
        WorkspacePane::Tags => "ux_tags",
        WorkspacePane::Remotes => "ux_remotes",
        WorkspacePane::Stashes => "ux_stashes",
        _ => "ux_references",
    };
    ui.heading(app.i18n.t(lang, title_key));
    let repo = app.local_repository.as_ref().cloned();
    let overview = app.history.overview.clone();
    let local_ok = repo.as_ref().map(|repo| {
        app.local_repo_input.as_str() == repo.root.to_string_lossy().as_ref()
    }).unwrap_or(false);
    let can_query = local_ok && !app.local_loading && !app.local_action_loading;
    if ui
        .add_enabled(
            can_query && !app.history.loading && !app.history.ref_loading,
            egui::Button::new(app.i18n.t(lang, "ux_refresh_refs")),
        )
        .clicked()
    {
        app.load_advanced_history(ctx.clone());
    }
    if app.history.loading {
        ui.spinner();
    }
    if let Some(error) = &app.history.error {
        ui.colored_label(egui::Color32::from_rgb(205, 55, 66), error);
    }

    egui::ScrollArea::vertical()
        .id_salt("gc_refs_main_scroll")
        .show(ui, |ui| {
            match pane {
                WorkspacePane::Branches | WorkspacePane::Remotes => {
                    if let Some(repository) = &repo {
                        let remote = pane == WorkspacePane::Remotes;
                        let mut total = 0;
                        for branch in repository.branches.iter().filter(|b| b.remote == remote) {
                            total += 1;
                            ui.horizontal(|ui| {
                                if !remote && branch.name == repository.head {
                                    ui.strong("●");
                                } else {
                                    ui.label("○");
                                }
                                ui.monospace(&branch.name);
                                ui.label(format!("· {}", &branch.commit[..8.min(branch.commit.len())]));
                            });
                        }
                        if total == 0 {
                            ui.label(app.i18n.t(lang, "ux_empty_list"));
                        }
                    }
                }
                WorkspacePane::Tags => {
                    if let Some(overview) = &overview {
                        render_readonly_rows(ui, &overview.tags);
                    }
                }
                WorkspacePane::Stashes => {
                    if let Some(overview) = &overview {
                        render_readonly_rows(ui, &overview.stash);
                    }
                }
                _ => {}
            }

            if pane == WorkspacePane::Branches || pane == WorkspacePane::Tags {
                ui.add_space(16.0);
                if let Some(overview) = overview {
                    let kind = if pane == WorkspacePane::Branches {
                        RefKind::Branch
                    } else {
                        RefKind::Tag
                    };
                    if app.history.ref_kind != kind {
                        app.cancel_local_ref();
                        app.history.ref_kind = kind;
                    }
                    render_ref_creation_panel(app, ui, ctx, &overview, can_query);
                } else {
                    ui.label(app.i18n.t(lang, "ux_load_refs_hint"));
                }
            } else {
                ui.add_space(12.0);
                ui.label(app.i18n.t(lang, "ux_read_only_section"));
            }
        });
}

/// Muestra nombres de referencias con fuente legible y líneas espaciadas.
#[cfg(not(target_arch = "wasm32"))]
fn render_readonly_rows(ui: &mut egui::Ui, values: &[String]) {
    if values.is_empty() {
        ui.label("—");
        return;
    }
    for value in values {
        ui.horizontal(|ui| {
            ui.label("○");
            ui.monospace(value);
        });
    }
}

/// Pantalla de conexión GitHub presentada sólo al abrir una función remota.
///
/// El campo del token conserva el comportamiento de seguridad anterior:
/// no se guarda en preferencias y no se comparte con MCP sin permiso.
fn render_github_connection(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    ui.heading(app.i18n.t(lang, "welcome_title"));
    ui.label(app.i18n.t(lang, "local_without_github_hint"));
    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.saved_token)
                .password(true)
                .desired_width(350.0)
                .hint_text("ghp_…"),
        );
        if ui.button(app.i18n.t(lang, "auth_connect_btn")).clicked() {
            app.trigger_github_sync(ctx.clone());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La navegación comienza en Archivos; nunca en una escritura destructiva.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn workspace_defaults_to_file_status() {
        assert_eq!(WorkspacePane::default(), WorkspacePane::FileStatus);
    }

    /// Visitar una subpantalla sólo cambia el destino visual, sin tocar Git.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn workspace_pane_is_not_a_git_operation() {
        let mut app = GitCraftApp::default();
        app.workspace_pane = WorkspacePane::History;
        assert!(app.local_action_preview.is_none());
        assert!(app.history.ref_preview.is_none());
        app.workspace_pane = WorkspacePane::Branches;
        assert_eq!(app.workspace_pane, WorkspacePane::Branches);
    }
}
