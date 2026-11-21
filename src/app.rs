//! Controlador principal de GitCraft.
//!
//! Conserva la información de las pantallas, coordina peticiones a GitHub y recibe
//! respuestas de tareas en segundo plano. Cuando Rust muestra `Result<T, String>`,
//! significa que una operación puede terminar con datos (`Ok`) o con un error (`Err`).
//! Las peticiones llevan marcadores para que una respuesta antigua no cambie otra pantalla.

use crate::github::{fetch_user_and_repos, GitHubRepo, GitHubUser};
use crate::github_manage::{
    close_issue, create_issue, create_pr, create_release, fetch_management_data, merge_pr,
    rerun_workflow, RepoManagementData,
};
use crate::i18n::{I18nManager, Language};
// Las funciones de Git local sólo se compilan en escritorio (macOS/Linux/Windows).
#[cfg(not(target_arch = "wasm32"))]
use crate::local_git::{inspect_repository, read_diff, LocalDiffTarget, LocalRepository};
#[cfg(not(target_arch = "wasm32"))]
use crate::local_git_actions::{
    execute_confirmed_action, preview_action, LocalAction, LocalActionPreview,
};
// E3-A recopila historial y detalles mediante consultas Git sin escritura.
#[cfg(not(target_arch = "wasm32"))]
use crate::local_git_history::{blame_file, compare_commits, inspect_history, GitHistoryOverview};
#[cfg(not(target_arch = "wasm32"))]
use crate::local_git_refs::{
    execute_ref, preview_ref, RefCreated, RefCreation, RefKind, RefPreview,
};
use crate::mcp_client::{mcp_call_tool, mcp_discover_tools, McpTool, McpTransport};
use crate::repo_ops::{
    commit_file_push, create_branch, fetch_branches, fetch_directory, fetch_file_content,
    GitBranch, RepoEntry,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::ui_local_history::{HistoryDetailRequest, HistoryUiState};
use crate::ui_shell::render_app_shell;
#[cfg(not(target_arch = "wasm32"))]
use crate::ui_shell::WorkspacePane;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveTab {
    Repositories,
    // Git local funciona incluso sin sesión de GitHub.
    LocalGit,
    Changes,
    PullRequests,
    Issues,
    Actions,
    Releases,
    Mcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoSortColumn {
    Name,
    Visibility,
    Description,
    CreatedAt,
    UpdatedAt,
}

/// Identifica la petición que obtuvo un resultado asíncrono.
/// Un resultado antiguo sólo es aplicable si pertenece al repositorio y petición actuales.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRequestKey {
    /// Número único que permite distinguir peticiones sucesivas, aunque sean al mismo archivo.
    pub id: u64,
    /// Identificador completo del repositorio, por ejemplo «Samafra/gitcraft».
    pub repo: String,
    /// Rama consultada; puede quedar vacía cuando no importa para esta petición.
    pub branch: String,
    /// Carpeta o archivo consultado; puede quedar vacío si no aplica.
    pub path: String,
}

/// Resultados que llegan a la interfaz desde tareas ejecutadas en paralelo.
/// Cada respuesta de un repositorio incluye su identidad para evitar cruces de datos.
pub enum AsyncEvent {
    UserAndRepos(u64, Result<(GitHubUser, Vec<GitHubRepo>), String>),
    Branches(RepoRequestKey, Result<Vec<GitBranch>, String>),
    Directory(RepoRequestKey, Result<Vec<RepoEntry>, String>),
    FileLoaded(RepoRequestKey, Result<(String, String), String>),
    CommitDone(RepoRequestKey, Result<String, String>),
    BranchCreated(RepoRequestKey, Result<(), String>),
    ManagementLoaded(RepoRequestKey, Result<RepoManagementData, String>),
    MgmtActionDone(String, Result<(), String>),
    McpToolsLoaded(Result<Vec<McpTool>, String>),
    McpCallResult(Result<String, String>),
    // El identificador permite descartar una lectura local obsoleta.
    #[cfg(not(target_arch = "wasm32"))]
    LocalRepositoryLoaded(u64, Result<LocalRepository, String>),
    #[cfg(not(target_arch = "wasm32"))]
    LocalDiffLoaded(u64, Result<String, String>),
    /// E2: vista previa sin escritura, identificada por su generación.
    #[cfg(not(target_arch = "wasm32"))]
    LocalActionPreviewLoaded(u64, Result<LocalActionPreview, String>),
    /// E2: resultado de una acción confirmada con actualización de datos.
    #[cfg(not(target_arch = "wasm32"))]
    LocalActionFinished(u64, LocalAction, Result<LocalRepository, String>),
    /// E3-A: lectura de grafo, etiquetas, reflog, stash y ficheros seguidos.
    #[cfg(not(target_arch = "wasm32"))]
    HistoryLoaded(u64, Result<GitHistoryOverview, String>),
    /// E3-A: resultado de comparación o blame, sin modificar Git.
    #[cfg(not(target_arch = "wasm32"))]
    HistoryDetailLoaded(u64, Result<String, String>),
    /// E3-B: vista previa de una creación de referencia local.
    #[cfg(not(target_arch = "wasm32"))]
    RefPreviewLoaded(u64, Result<RefPreview, String>),
    /// E3-B: resultado de la escritura explícitamente confirmada.
    #[cfg(not(target_arch = "wasm32"))]
    RefCreationFinished(u64, Result<RefCreated, String>),
}

/// Estado general de GitCraft: preferencias, sesión y datos que utiliza la interfaz.
/// `serde` guarda sólo las preferencias autorizadas; los campos con `skip` no se persisten.
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct GitCraftApp {
    pub language: Language,
    pub active_tab: ActiveTab,
    pub dark_mode: bool,
    pub show_mcp_sidebar: bool,
    // El PAT sólo vive en memoria: nunca se serializa en las preferencias de eframe.
    #[serde(skip)]
    pub saved_token: String,
    #[serde(skip)]
    pub i18n: I18nManager,
    #[serde(skip)]
    pub is_loading: bool,
    #[serde(skip)]
    pub status_notice: Option<(String, bool)>,
    #[serde(skip)]
    pub user: Option<GitHubUser>,
    #[serde(skip)]
    pub repos: Vec<GitHubRepo>,
    #[serde(skip)]
    pub selected_repo: Option<GitHubRepo>,
    #[serde(skip)]
    pub repo_filter: String,
    pub repo_sort_col: RepoSortColumn,
    pub repo_sort_asc: bool,
    #[serde(skip)]
    pub branches: Vec<GitBranch>,
    #[serde(skip)]
    pub current_branch: String,
    #[serde(skip)]
    pub new_branch_name: String,
    #[serde(skip)]
    pub current_path: String,
    #[serde(skip)]
    pub dir_entries: Vec<RepoEntry>,
    #[serde(skip)]
    pub opened_file_path: Option<String>,
    #[serde(skip)]
    pub opened_file_sha: String,
    #[serde(skip)]
    pub opened_file_content: String,
    #[serde(skip)]
    pub commit_message: String,
    #[serde(skip)]
    pub mgmt_data: RepoManagementData,
    #[serde(skip)]
    pub new_pr_title: String,
    #[serde(skip)]
    pub new_pr_head: String,
    #[serde(skip)]
    pub new_pr_base: String,
    #[serde(skip)]
    pub new_issue_title: String,
    #[serde(skip)]
    pub new_issue_body: String,
    #[serde(skip)]
    pub new_rel_tag: String,
    #[serde(skip)]
    pub new_rel_name: String,
    #[serde(skip)]
    pub new_rel_body: String,
    pub mcp_transport: McpTransport,
    pub mcp_endpoint: String,
    // Permiso temporal, por dirección exacta, para transmitir el PAT a un proceso MCP.
    // Nunca se guarda en disco ni se conserva tras reiniciar la aplicación.
    #[serde(skip)]
    pub mcp_token_authorized_endpoint: Option<String>,
    #[serde(skip)]
    pub mcp_tools: Vec<McpTool>,
    #[serde(skip)]
    pub mcp_selected_tool: String,
    #[serde(skip)]
    pub mcp_tool_args: String,
    #[serde(skip)]
    pub mcp_last_output: String,
    // E1: todos estos datos locales son temporales, no se guardan con serde.
    // cfg impide incluir funciones de sistema de archivos en la versión Web.
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub local_repo_input: String,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub local_repository: Option<LocalRepository>,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub local_loading: bool,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub local_error: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub local_diff_target: Option<LocalDiffTarget>,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub local_diff_text: String,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub local_diff_loading: bool,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    local_generation: u64,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    local_diff_generation: u64,
    // E2 no conserva acciones pendientes ni mensajes de commit en preferencias.
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub local_action_preview: Option<LocalActionPreview>,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub local_action_loading: bool,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub local_action_notice: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub local_commit_message: String,
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    local_action_generation: u64,
    // E3-A vive sólo en RAM: el grafo y los archivos no se serializan.
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub history: HistoryUiState,
    // La navegación Git al estilo SourceTree no se persiste entre sesiones.
    #[cfg(not(target_arch = "wasm32"))]
    #[serde(skip)]
    pub workspace_pane: WorkspacePane,
    // Identificadores de las peticiones más recientes; pertenecen sólo a esta sesión.
    #[serde(skip)]
    pub request_generation: u64,
    #[serde(skip)]
    pub auth_generation: u64,
    #[serde(skip)]
    pub branches_request: Option<RepoRequestKey>,
    #[serde(skip)]
    pub directory_request: Option<RepoRequestKey>,
    #[serde(skip)]
    pub file_request: Option<RepoRequestKey>,
    #[serde(skip)]
    pub commit_request: Option<RepoRequestKey>,
    #[serde(skip)]
    pub branch_create_request: Option<RepoRequestKey>,
    #[serde(skip)]
    pub management_request: Option<RepoRequestKey>,
    #[serde(skip)]
    tx: Option<Sender<AsyncEvent>>,
    #[serde(skip)]
    rx: Option<Receiver<AsyncEvent>>,
}

/// Construye un estado inicial sin conexión y con permisos MCP desactivados.
impl Default for GitCraftApp {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            language: Language::Es,
            // Abrir directamente en Git local sin solicitar login de GitHub.
            #[cfg(not(target_arch = "wasm32"))]
            active_tab: ActiveTab::LocalGit,
            #[cfg(target_arch = "wasm32")]
            active_tab: ActiveTab::Repositories,
            dark_mode: false,
            show_mcp_sidebar: false,
            saved_token: String::new(),
            i18n: I18nManager::default(),
            is_loading: false,
            status_notice: None,
            user: None,
            repos: Vec::new(),
            selected_repo: None,
            repo_filter: String::new(),
            repo_sort_col: RepoSortColumn::UpdatedAt,
            repo_sort_asc: false,
            branches: Vec::new(),
            current_branch: "main".into(),
            new_branch_name: String::new(),
            current_path: String::new(),
            dir_entries: Vec::new(),
            opened_file_path: None,
            opened_file_sha: String::new(),
            opened_file_content: String::new(),
            commit_message: String::new(),
            mgmt_data: RepoManagementData::default(),
            new_pr_title: String::new(),
            new_pr_head: String::new(),
            new_pr_base: "main".into(),
            new_issue_title: String::new(),
            new_issue_body: String::new(),
            new_rel_tag: String::new(),
            new_rel_name: String::new(),
            new_rel_body: String::new(),
            mcp_transport: McpTransport::Stdio,
            mcp_endpoint: "npx -y @modelcontextprotocol/server-github".into(),
            mcp_token_authorized_endpoint: None,
            mcp_tools: Vec::new(),
            mcp_selected_tool: String::new(),
            mcp_tool_args: "{}".into(),
            mcp_last_output: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            local_repo_input: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            local_repository: None,
            #[cfg(not(target_arch = "wasm32"))]
            local_loading: false,
            #[cfg(not(target_arch = "wasm32"))]
            local_error: None,
            #[cfg(not(target_arch = "wasm32"))]
            local_diff_target: None,
            #[cfg(not(target_arch = "wasm32"))]
            local_diff_text: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            local_diff_loading: false,
            #[cfg(not(target_arch = "wasm32"))]
            local_generation: 0,
            #[cfg(not(target_arch = "wasm32"))]
            local_diff_generation: 0,
            #[cfg(not(target_arch = "wasm32"))]
            local_action_preview: None,
            #[cfg(not(target_arch = "wasm32"))]
            local_action_loading: false,
            #[cfg(not(target_arch = "wasm32"))]
            local_action_notice: None,
            #[cfg(not(target_arch = "wasm32"))]
            local_commit_message: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            local_action_generation: 0,
            #[cfg(not(target_arch = "wasm32"))]
            history: HistoryUiState::default(),
            #[cfg(not(target_arch = "wasm32"))]
            workspace_pane: WorkspacePane::default(),
            request_generation: 0,
            auth_generation: 0,
            branches_request: None,
            directory_request: None,
            file_request: None,
            commit_request: None,
            branch_create_request: None,
            management_request: None,
            tx: Some(tx),
            rx: Some(rx),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_task<F>(ctx: egui::Context, fut: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(fut);
        ctx.request_repaint();
    });
}

#[cfg(target_arch = "wasm32")]
fn spawn_task<F>(ctx: egui::Context, fut: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(async move {
        fut.await;
        ctx.request_repaint();
    });
}

impl GitCraftApp {
    /// Restaura preferencias al abrir la aplicación, pero nunca recupera el token antiguo.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app: Self = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        let (tx, rx) = channel();
        app.tx = Some(tx);
        app.rx = Some(rx);
        // La aplicación puede recuperar una pestaña remota de preferencias
        // antiguas aunque el PAT ya no esté cargado. Empezar en la copia de
        // trabajo evita una pantalla de login innecesaria en escritorio.
        #[cfg(not(target_arch = "wasm32"))]
        if app.user.is_none() {
            app.active_tab = ActiveTab::LocalGit;
        }
        app.apply_theme(&cc.egui_ctx);
        if !app.saved_token.trim().is_empty() {
            app.trigger_github_sync(cc.egui_ctx.clone());
        }
        app
    }
    /// Ajusta colores claros u oscuros de todos los controles de la aplicación.
    pub(crate) fn apply_theme(&self, ctx: &egui::Context) {
        // Ambos temas mantienen contraste de texto alto; no se utilizan
        // colores pálidos sobre fondos similares para información importante.
        let mut visuals = if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        if self.dark_mode {
            visuals.override_text_color = Some(egui::Color32::from_rgb(238, 241, 247));
            visuals.panel_fill = egui::Color32::from_rgb(27, 33, 43);
            visuals.window_fill = egui::Color32::from_rgb(32, 38, 49);
            visuals.extreme_bg_color = egui::Color32::from_rgb(21, 26, 34);
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 53, 66);
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(60, 72, 91);
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(55, 86, 132);
            visuals.selection.bg_fill = egui::Color32::from_rgb(56, 96, 151);
            visuals.selection.stroke.color = egui::Color32::WHITE;
        } else {
            visuals.override_text_color = Some(egui::Color32::from_rgb(30, 40, 56));
            visuals.panel_fill = egui::Color32::from_rgb(244, 247, 251);
            visuals.window_fill = egui::Color32::WHITE;
            visuals.extreme_bg_color = egui::Color32::WHITE;
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(231, 237, 245);
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(211, 226, 246);
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(187, 211, 242);
            visuals.selection.bg_fill = egui::Color32::from_rgb(178, 207, 244);
            visuals.selection.stroke.color = egui::Color32::from_rgb(18, 48, 93);
        }
        ctx.set_visuals(visuals);

        // Un espaciado uniforme facilita localizar grupos y botones.
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 7.0);
        style.spacing.button_padding = egui::vec2(11.0, 6.0);
        ctx.set_style(style);
    }
    /// Genera un marcador único para una nueva petición a GitHub.
    /// El contador se incrementa para que una respuesta anterior nunca suplante a la última.
    fn new_repo_request(&mut self, repo: &str, branch: &str, path: &str) -> RepoRequestKey {
        self.request_generation = self.request_generation.wrapping_add(1);
        RepoRequestKey {
            id: self.request_generation,
            repo: repo.to_string(),
            branch: branch.to_string(),
            path: path.to_string(),
        }
    }

    /// Comprueba que la respuesta corresponde tanto al repositorio seleccionado
    /// como al marcador de la petición más reciente de esa clase.
    fn is_current_repo_request(
        &self,
        latest: &Option<RepoRequestKey>,
        key: &RepoRequestKey,
    ) -> bool {
        latest.as_ref() == Some(key)
            && self
                .selected_repo
                .as_ref()
                .map(|repo| repo.full_name.as_str())
                == Some(key.repo.as_str())
    }

    /// Cierra la sesión, borra el PAT en memoria e invalida respuestas pendientes.
    pub fn disconnect_github(&mut self) {
        self.auth_generation = self.auth_generation.wrapping_add(1);
        self.is_loading = false;
        self.saved_token.clear();
        self.mcp_token_authorized_endpoint = None;
        self.user = None;
        self.repos.clear();
        self.selected_repo = None;
        self.branches.clear();
        self.dir_entries.clear();
        self.opened_file_path = None;
        self.opened_file_sha.clear();
        self.opened_file_content.clear();
        self.branches_request = None;
        self.directory_request = None;
        self.file_request = None;
        self.commit_request = None;
        self.branch_create_request = None;
        self.management_request = None;
        self.mgmt_data = RepoManagementData::default();
    }

    /// Abre la ruta introducida utilizando exclusivamente consultas Git locales.
    ///
    /// La tarea se ejecuta fuera de la interfaz. Se incrementa un contador
    /// cada vez que el usuario abre una carpeta, para descartar resultados viejos.
    /// No se utilizan tokens, red ni operaciones de escritura Git.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_local_repository(&mut self, ctx: egui::Context) {
        self.local_generation = self.local_generation.wrapping_add(1);
        self.local_diff_generation = self.local_diff_generation.wrapping_add(1);
        self.local_action_generation = self.local_action_generation.wrapping_add(1);
        self.history.invalidate();
        self.local_action_preview = None;
        self.local_action_notice = None;
        self.local_action_loading = false;
        self.local_loading = true;
        self.local_diff_loading = false;
        self.local_error = None;
        self.local_repository = None;
        self.local_diff_target = None;
        self.local_diff_text.clear();

        // String posee sus caracteres: lo movemos a la tarea para que Rust
        // no necesite prestar la caja de texto mientras Git está consultando.
        let path = self.local_repo_input.clone();
        let generation = self.local_generation;
        let tx = self.tx.clone().unwrap();
        spawn_task(ctx, async move {
            let result = inspect_repository(&path);
            let _ = tx.send(AsyncEvent::LocalRepositoryLoaded(generation, result));
        });
    }

    /// Actualiza el mismo repositorio sin alterar su rama ni sus archivos.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn refresh_local_repository(&mut self, ctx: egui::Context) {
        if let Some(repository) = &self.local_repository {
            self.local_repo_input = repository.root.to_string_lossy().into_owned();
            self.open_local_repository(ctx);
        }
    }

    /// Solicita un diff sólo cuando hay un repositorio local abierto.
    ///
    /// Las respuestas llevan un identificador: si se solicita otro diff antes
    /// de recibir el primero, el resultado antiguo no reemplaza la pantalla.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_local_diff(&mut self, ctx: egui::Context, target: LocalDiffTarget) {
        let Some(repository) = &self.local_repository else {
            return;
        };
        let root = repository.root.clone();
        let tx = self.tx.clone().unwrap();

        self.local_diff_generation = self.local_diff_generation.wrapping_add(1);
        let generation = self.local_diff_generation;
        self.local_diff_target = Some(target.clone());
        self.local_diff_text.clear();
        self.local_diff_loading = true;
        self.local_error = None;
        spawn_task(ctx, async move {
            let result = read_diff(&root, &target);
            let _ = tx.send(AsyncEvent::LocalDiffLoaded(generation, result));
        });
    }

    /// E2: prepara una vista previa sin ejecutar ninguna escritura Git.
    ///
    /// Se utiliza la ruta de la selección actual y se anulan previews anteriores.
    /// El identificador evita aplicar respuestas de otro repositorio.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn preview_local_action(&mut self, ctx: egui::Context, action: LocalAction) {
        let Some(repository) = &self.local_repository else {
            return;
        };
        if self.local_loading || self.local_action_loading || self.history.ref_loading {
            return;
        }
        let root = repository.root.clone();
        let tx = self.tx.clone().unwrap();
        self.local_action_generation = self.local_action_generation.wrapping_add(1);
        let generation = self.local_action_generation;
        self.local_action_loading = true;
        self.local_action_preview = None;
        self.local_action_notice = None;
        self.local_error = None;
        spawn_task(ctx, async move {
            let result = preview_action(&root, action);
            let _ = tx.send(AsyncEvent::LocalActionPreviewLoaded(generation, result));
        });
    }

    /// E2: descarta una autorización no ejecutada sin tocar el repositorio.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn cancel_local_action(&mut self) {
        self.local_action_generation = self.local_action_generation.wrapping_add(1);
        self.local_action_preview = None;
        self.local_action_loading = false;
    }

    /// E2: ejecuta una operación solamente tras confirmar su vista previa.
    ///
    /// El motor comprueba que índice, HEAD y archivos no hayan cambiado desde
    /// el preview. Si cambiaron, devuelve un error y no hace la escritura.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn confirm_local_action(&mut self, ctx: egui::Context) {
        if self.local_action_loading || self.local_loading || self.history.ref_loading {
            return;
        }
        let Some(preview) = self.local_action_preview.take() else {
            return;
        };
        let current_root = self.local_repository.as_ref().map(|repo| &repo.root);
        if current_root != Some(&preview.root) {
            self.local_error = Some("El repositorio ha cambiado; revisa de nuevo.".into());
            return;
        }
        if let LocalAction::Commit(message) = &preview.action {
            if message.as_str() != self.local_commit_message.trim() {
                self.local_error = Some("El mensaje cambió; genera otra vista previa.".into());
                return;
            }
        }
        let action = preview.action.clone();
        let tx = self.tx.clone().unwrap();
        self.local_action_generation = self.local_action_generation.wrapping_add(1);
        let generation = self.local_action_generation;
        self.local_action_loading = true;
        self.local_error = None;
        spawn_task(ctx, async move {
            let result = execute_confirmed_action(&preview);
            let _ = tx.send(AsyncEvent::LocalActionFinished(generation, action, result));
        });
    }

    /// E3-A: obtiene el grafo y sus referencias exclusivamente mediante lectura.
    ///
    /// La petición usa un contador para que resultados de otra carpeta no
    /// aparezcan después de cambiar de repositorio o actualizar la pantalla.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_advanced_history(&mut self, ctx: egui::Context) {
        let Some(repository) = &self.local_repository else {
            return;
        };
        if self.local_loading
            || self.local_action_loading
            || self.history.loading
            || self.history.ref_loading
        {
            return;
        }
        let root = repository.root.clone();
        self.history.invalidate();
        self.history.loading = true;
        let generation = self.history.overview_generation;
        let tx = self.tx.clone().unwrap();
        spawn_task(ctx, async move {
            let result = inspect_history(&root);
            let _ = tx.send(AsyncEvent::HistoryLoaded(generation, result));
        });
    }

    /// E3-A: compara SHA seleccionados o consulta blame sobre un archivo seguido.
    ///
    /// Aunque los selectores ya restringen las opciones visibles, se vuelve
    /// a comprobar su pertenencia a la instantánea en memoria antes de pasar
    /// la petición al motor, evitando entradas obsoletas o manipuladas.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_advanced_detail(&mut self, ctx: egui::Context, request: HistoryDetailRequest) {
        let Some(repository) = &self.local_repository else {
            return;
        };
        if self.local_loading
            || self.local_action_loading
            || self.history.detail_loading
            || self.history.ref_loading
        {
            return;
        }
        let Some(overview) = &self.history.overview else {
            return;
        };
        if overview.root != repository.root {
            self.history.error = Some("La consulta pertenece a otro repositorio.".into());
            return;
        }
        let permitted = match &request {
            HistoryDetailRequest::Comparison { left, right } => {
                overview
                    .revisions
                    .iter()
                    .any(|rev| rev.sha.as_str() == left.as_str())
                    && overview
                        .revisions
                        .iter()
                        .any(|rev| rev.sha.as_str() == right.as_str())
                    && left != right
            }
            HistoryDetailRequest::Blame(file) => {
                overview.tracked_files.iter().any(|path| path == file)
            }
        };
        if !permitted {
            self.history.error = Some("La selección ya no es válida; recarga E3.".into());
            return;
        }

        let root = repository.root.clone();
        self.history.detail_generation = self.history.detail_generation.wrapping_add(1);
        let generation = self.history.detail_generation;
        self.history.detail_loading = true;
        self.history.detail.clear();
        self.history.error = None;
        self.history.detail_label = match &request {
            HistoryDetailRequest::Comparison { left, right } => {
                format!("Diff {} → {}", &left[..8], &right[..8])
            }
            HistoryDetailRequest::Blame(file) => format!("Blame: {}", file),
        };

        let tx = self.tx.clone().unwrap();
        spawn_task(ctx, async move {
            let result = match request {
                HistoryDetailRequest::Comparison { left, right } => {
                    compare_commits(&root, &left, &right)
                }
                HistoryDetailRequest::Blame(file) => blame_file(&root, &file),
            };
            let _ = tx.send(AsyncEvent::HistoryDetailLoaded(generation, result));
        });
    }

    /// E3-B: comprueba sin escribir una nueva rama o etiqueta.
    ///
    /// El SHA debe pertenecer al historial que ya abrió esta instancia.
    /// Se invalida toda vista previa anterior y se identifican las respuestas
    /// con un contador para ignorar operaciones de otro repositorio.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn preview_local_ref(&mut self, ctx: egui::Context) {
        let Some(repository) = &self.local_repository else {
            return;
        };
        if self.local_loading
            || self.local_action_loading
            || self.history.loading
            || self.history.ref_loading
        {
            return;
        }
        let Some(overview) = &self.history.overview else {
            return;
        };
        if overview.root != repository.root
            || !overview
                .revisions
                .iter()
                .any(|revision| revision.sha == self.history.ref_target)
        {
            self.history.ref_error =
                Some("El commit seleccionado ya no pertenece al historial cargado.".into());
            return;
        }

        let action = RefCreation {
            kind: self.history.ref_kind,
            name: self.history.ref_name.clone(),
            target: self.history.ref_target.clone(),
        };
        let root = repository.root.clone();
        let tx = self.tx.clone().unwrap();
        self.history.ref_generation = self.history.ref_generation.wrapping_add(1);
        let generation = self.history.ref_generation;
        self.history.ref_preview = None;
        self.history.ref_loading = true;
        self.history.ref_error = None;
        spawn_task(ctx, async move {
            let result = preview_ref(&root, action);
            let _ = tx.send(AsyncEvent::RefPreviewLoaded(generation, result));
        });
    }

    /// E3-B: revoca la confirmación antes de ejecutar ninguna escritura.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn cancel_local_ref(&mut self) {
        self.history.ref_generation = self.history.ref_generation.wrapping_add(1);
        self.history.ref_preview = None;
        self.history.ref_loading = false;
        self.history.ref_error = None;
    }

    /// E3-B: ejecuta sólo la referencia incluida en la vista previa aceptada.
    ///
    /// El motor vuelve a verificar HEAD, índice, estado y todas las referencias.
    /// Si han cambiado, la creación se cancela antes de tocar el repositorio.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn confirm_local_ref(&mut self, ctx: egui::Context) {
        if self.local_loading || self.local_action_loading || self.history.ref_loading {
            return;
        }
        let Some(preview) = self.history.ref_preview.take() else {
            return;
        };
        let same_root =
            self.local_repository.as_ref().map(|repo| &repo.root) == Some(&preview.root);
        let same_selection = preview.action.kind == self.history.ref_kind
            && preview.action.name == self.history.ref_name
            && preview.action.target == self.history.ref_target;
        if !same_root || !same_selection {
            self.history.ref_error =
                Some("La carpeta o la selección cambió. Genera otra vista previa.".into());
            return;
        }
        let tx = self.tx.clone().unwrap();
        self.history.ref_generation = self.history.ref_generation.wrapping_add(1);
        let generation = self.history.ref_generation;
        self.history.ref_loading = true;
        self.history.ref_error = None;
        spawn_task(ctx, async move {
            let result = execute_ref(&preview);
            let _ = tx.send(AsyncEvent::RefCreationFinished(generation, result));
        });
    }

    /// Inicia una conexión nueva: sólo su respuesta puede establecer la sesión.
    pub fn trigger_github_sync(&mut self, ctx: egui::Context) {
        self.is_loading = true;
        self.auth_generation = self.auth_generation.wrapping_add(1);
        let generation = self.auth_generation;
        let (token, tx) = (self.saved_token.clone(), self.tx.clone().unwrap());
        spawn_task(ctx, async move {
            let result = fetch_user_and_repos(token).await;
            let _ = tx.send(AsyncEvent::UserAndRepos(generation, result));
        });
    }

    /// Cambia a la rama inicial de un repositorio recién seleccionado y carga sus datos.
    /// Esta función SÍ selecciona la rama predeterminada, pero no debe usarse para refrescar.
    pub fn load_branches_and_root(&mut self, ctx: egui::Context) {
        let Some(repo) = &self.selected_repo else {
            return;
        };
        self.current_branch = repo.default_branch.clone();
        self.new_pr_base = repo.default_branch.clone();
        self.current_path.clear();
        self.opened_file_path = None;
        self.opened_file_sha.clear();
        self.opened_file_content.clear();
        self.file_request = None;
        // Limpiamos información antigua mientras se descargan los datos del nuevo repo.
        self.branches.clear();
        self.dir_entries.clear();
        self.mgmt_data = RepoManagementData::default();
        self.refresh_branches(ctx.clone());
        self.load_directory(ctx.clone());
        self.load_management(ctx);
    }

    /// Vuelve a leer las ramas sin cambiar la rama actualmente seleccionada.
    pub fn refresh_branches(&mut self, ctx: egui::Context) {
        let Some(repo) = &self.selected_repo else {
            return;
        };
        let name = repo.full_name.clone();
        let token = self.saved_token.clone();
        let tx = self.tx.clone().unwrap();
        let key = self.new_repo_request(&name, "", "");
        self.branches_request = Some(key.clone());
        spawn_task(ctx, async move {
            let result = fetch_branches(token, name).await;
            let _ = tx.send(AsyncEvent::Branches(key, result));
        });
    }

    /// Lee una carpeta de la rama seleccionada y marca qué petición sigue vigente.
    pub fn load_directory(&mut self, ctx: egui::Context) {
        let Some(repo) = &self.selected_repo else {
            return;
        };
        let name = repo.full_name.clone();
        let branch = self.current_branch.clone();
        let path = self.current_path.clone();
        let token = self.saved_token.clone();
        let tx = self.tx.clone().unwrap();
        let key = self.new_repo_request(&name, &branch, &path);
        self.directory_request = Some(key.clone());
        // Cambiar de directorio hace obsoleta cualquier carga de archivo pendiente.
        self.file_request = None;
        spawn_task(ctx, async move {
            let result = fetch_directory(token, name, branch, path).await;
            let _ = tx.send(AsyncEvent::Directory(key, result));
        });
    }

    /// Abre un archivo remoto; las respuestas de archivos anteriores se descartan.
    pub fn open_file(&mut self, ctx: egui::Context, path: String) {
        let Some(repo) = &self.selected_repo else {
            return;
        };
        let name = repo.full_name.clone();
        let branch = self.current_branch.clone();
        let token = self.saved_token.clone();
        let tx = self.tx.clone().unwrap();
        let key = self.new_repo_request(&name, &branch, &path);
        self.file_request = Some(key.clone());
        spawn_task(ctx, async move {
            let result = fetch_file_content(token, name, branch, path).await;
            let _ = tx.send(AsyncEvent::FileLoaded(key, result));
        });
    }

    /// Crea una rama en GitHub conservando su nombre y repositorio originales.
    /// Aunque el usuario cambie de repositorio antes de terminar, no se modificará el actual.
    pub fn trigger_create_branch(&mut self, ctx: egui::Context) {
        let Some(repo) = &self.selected_repo else {
            return;
        };
        let sha = self
            .branches
            .iter()
            .find(|branch| branch.name == self.current_branch)
            .map(|branch| branch.commit.sha.clone())
            .unwrap_or_default();
        let name = repo.full_name.clone();
        let branch = self.new_branch_name.trim().to_string();
        let (token, tx) = (self.saved_token.clone(), self.tx.clone().unwrap());
        let key = self.new_repo_request(&name, "", &branch);
        self.branch_create_request = Some(key.clone());
        spawn_task(ctx, async move {
            let result = create_branch(token, name, branch, sha).await;
            let _ = tx.send(AsyncEvent::BranchCreated(key, result));
        });
    }

    /// Tras crear una rama, cambia de selección y cierra el archivo abierto.
    /// No llama a load_branches_and_root porque esa función volvería a main/default.
    fn select_created_branch(&mut self, branch: &str) {
        self.current_branch = branch.to_string();
        self.current_path.clear();
        self.opened_file_path = None;
        self.opened_file_content.clear();
        self.opened_file_sha.clear();
        self.file_request = None;
    }

    /// Publica un archivo remoto; el SHA resultante sólo se aplica al editor original.
    pub fn trigger_commit_push(&mut self, ctx: egui::Context) {
        let (Some(repo), Some(path)) = (&self.selected_repo, &self.opened_file_path) else {
            return;
        };
        let msg = if self.commit_message.trim().is_empty() {
            format!("Update {}", path)
        } else {
            self.commit_message.clone()
        };
        let name = repo.full_name.clone();
        let path = path.clone();
        let branch = self.current_branch.clone();
        let sha = self.opened_file_sha.clone();
        let content = self.opened_file_content.clone();
        let (token, tx) = (self.saved_token.clone(), self.tx.clone().unwrap());
        let key = self.new_repo_request(&name, &branch, &path);
        self.commit_request = Some(key.clone());
        spawn_task(ctx, async move {
            let result = commit_file_push(token, name, branch, path, sha, content, msg).await;
            let _ = tx.send(AsyncEvent::CommitDone(key, result));
        });
    }

    /// Recupera los datos de gestión remota, asociando la respuesta al repositorio correcto.
    pub fn load_management(&mut self, ctx: egui::Context) {
        let Some(repo) = &self.selected_repo else {
            return;
        };
        let name = repo.full_name.clone();
        let token = self.saved_token.clone();
        let tx = self.tx.clone().unwrap();
        let key = self.new_repo_request(&name, "", "");
        self.management_request = Some(key.clone());
        spawn_task(ctx, async move {
            let result = fetch_management_data(token, name).await;
            let _ = tx.send(AsyncEvent::ManagementLoaded(key, result));
        });
    }
    /// Crea un Pull Request en GitHub con las ramas y título indicados en pantalla.
    pub fn trigger_create_pr(&mut self, ctx: egui::Context) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, ti, h, b, tx) = (
            self.saved_token.clone(),
            r.full_name.clone(),
            self.new_pr_title.clone(),
            self.new_pr_head.clone(),
            self.new_pr_base.clone(),
            self.tx.clone().unwrap(),
        );
        self.new_pr_title.clear();
        spawn_task(ctx, async move {
            let repo_name = f.clone();
            let result = create_pr(t, f, ti, h, b).await;
            let _ = tx.send(AsyncEvent::MgmtActionDone(repo_name, result));
        });
    }
    /// Fusiona un Pull Request por su número utilizando la API de GitHub.
    pub fn trigger_merge_pr(&mut self, ctx: egui::Context, num: u64) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, tx) = (
            self.saved_token.clone(),
            r.full_name.clone(),
            self.tx.clone().unwrap(),
        );
        spawn_task(ctx, async move {
            let repo_name = f.clone();
            let result = merge_pr(t, f, num).await;
            let _ = tx.send(AsyncEvent::MgmtActionDone(repo_name, result));
        });
    }
    /// Envía un Issue nuevo a GitHub y gestiona su respuesta asíncrona.
    pub fn trigger_create_issue(&mut self, ctx: egui::Context) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, ti, bo, tx) = (
            self.saved_token.clone(),
            r.full_name.clone(),
            self.new_issue_title.clone(),
            self.new_issue_body.clone(),
            self.tx.clone().unwrap(),
        );
        self.new_issue_title.clear();
        self.new_issue_body.clear();
        spawn_task(ctx, async move {
            let repo_name = f.clone();
            let result = create_issue(t, f, ti, bo).await;
            let _ = tx.send(AsyncEvent::MgmtActionDone(repo_name, result));
        });
    }
    /// Cierra un Issue existente por su número; no borra su historial.
    pub fn trigger_close_issue(&mut self, ctx: egui::Context, num: u64) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, tx) = (
            self.saved_token.clone(),
            r.full_name.clone(),
            self.tx.clone().unwrap(),
        );
        spawn_task(ctx, async move {
            let repo_name = f.clone();
            let result = close_issue(t, f, num).await;
            let _ = tx.send(AsyncEvent::MgmtActionDone(repo_name, result));
        });
    }
    /// Función de la interfaz que permite solicitar relanzar un workflow desde GitCraft.
    /// No se utiliza para las pruebas ni los despliegues de este proyecto.
    pub fn trigger_rerun_workflow(&mut self, ctx: egui::Context, id: u64) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, tx) = (
            self.saved_token.clone(),
            r.full_name.clone(),
            self.tx.clone().unwrap(),
        );
        spawn_task(ctx, async move {
            let repo_name = f.clone();
            let result = rerun_workflow(t, f, id).await;
            let _ = tx.send(AsyncEvent::MgmtActionDone(repo_name, result));
        });
    }
    /// Indica si el usuario ha autorizado compartir el token con el comando MCP actual.
    /// El permiso sólo existe durante esta sesión y se invalida al cambiar el destino.
    pub fn mcp_token_is_authorized(&self) -> bool {
        self.mcp_transport == McpTransport::Stdio
            && self.mcp_token_authorized_endpoint.as_deref() == Some(self.mcp_endpoint.as_str())
    }

    /// Devuelve el token sólo cuando existe consentimiento explícito para este servidor.
    /// Para cualquier servidor no autorizado se devuelve una cadena vacía.
    fn token_for_mcp(&self) -> String {
        if self.mcp_token_is_authorized() {
            self.saved_token.clone()
        } else {
            String::new()
        }
    }

    /// Consulta las herramientas expuestas por el servidor MCP elegido en la interfaz.
    /// Sólo entrega credenciales si se autorizaron explícitamente.
    pub fn trigger_mcp_discover(&mut self, ctx: egui::Context) {
        self.is_loading = true;
        let (tr, ep, tok, tx) = (
            self.mcp_transport,
            self.mcp_endpoint.clone(),
            self.token_for_mcp(),
            self.tx.clone().unwrap(),
        );
        spawn_task(ctx, async move {
            let _ = tx.send(AsyncEvent::McpToolsLoaded(
                mcp_discover_tools(tr, ep, tok).await,
            ));
        });
    }
    /// Ejecuta una herramienta MCP seleccionada con sus argumentos JSON.
    /// El proceso que recibe el token debe contar con permiso explícito.
    pub fn trigger_mcp_call(&mut self, ctx: egui::Context) {
        self.is_loading = true;
        let (tr, ep, tok, tool, args, tx) = (
            self.mcp_transport,
            self.mcp_endpoint.clone(),
            self.token_for_mcp(),
            self.mcp_selected_tool.clone(),
            self.mcp_tool_args.clone(),
            self.tx.clone().unwrap(),
        );
        spawn_task(ctx, async move {
            let _ = tx.send(AsyncEvent::McpCallResult(
                mcp_call_tool(tr, ep, tok, tool, args).await,
            ));
        });
    }
    /// Solicita publicar una Release en GitHub; esta operación es distinta de crear un tag Git local.
    pub fn trigger_create_release(&mut self, ctx: egui::Context) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, tg, nm, bo, tx) = (
            self.saved_token.clone(),
            r.full_name.clone(),
            self.new_rel_tag.clone(),
            self.new_rel_name.clone(),
            self.new_rel_body.clone(),
            self.tx.clone().unwrap(),
        );
        self.new_rel_tag.clear();
        self.new_rel_name.clear();
        self.new_rel_body.clear();
        spawn_task(ctx, async move {
            let repo_name = f.clone();
            let result = create_release(t, f, tg, nm, bo).await;
            let _ = tx.send(AsyncEvent::MgmtActionDone(repo_name, result));
        });
    }
}

/// Integración con `eframe`: guarda preferencias y actualiza la pantalla en cada ciclo.
impl eframe::App for GitCraftApp {
    /// Escribe las preferencias permitidas; gracias a `serde(skip)` no escribe el PAT.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Atiende respuestas pendientes y dibuja las pestañas actuales.
    /// `&mut self` permite cambiar el estado y `ctx` solicita repintar la interfaz.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut pending = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(ev) = rx.try_recv() {
                pending.push(ev);
            }
        }
        for ev in pending {
            self.is_loading = false;
            match ev {
                // Un login antiguo ya no puede restaurar una sesión cerrada.
                AsyncEvent::UserAndRepos(generation, result) => {
                    if generation != self.auth_generation {
                        continue;
                    }
                    match result {
                        Ok((user, repos)) => {
                            self.user = Some(user);
                            self.repos = repos;
                            self.status_notice = None;
                        }
                        Err(error) => self.status_notice = Some((error, true)),
                    }
                }
                // Sólo se aplican las ramas de la petición vigente para el repo visible.
                AsyncEvent::Branches(key, result) => {
                    if !self.is_current_repo_request(&self.branches_request, &key) {
                        continue;
                    }
                    match result {
                        Ok(branches) => {
                            if self.new_pr_head.is_empty()
                                || !branches
                                    .iter()
                                    .any(|branch| branch.name == self.new_pr_head)
                            {
                                self.new_pr_head = branches
                                    .iter()
                                    .find(|branch| branch.name != self.new_pr_base)
                                    .or_else(|| branches.first())
                                    .map(|branch| branch.name.clone())
                                    .unwrap_or_default();
                            }
                            self.branches = branches;
                        }
                        Err(error) => self.status_notice = Some((error, true)),
                    }
                }
                // Además del repo, la carpeta y la rama deben seguir siendo las mismas.
                AsyncEvent::Directory(key, result) => {
                    if !self.is_current_repo_request(&self.directory_request, &key)
                        || self.current_branch != key.branch
                        || self.current_path != key.path
                    {
                        continue;
                    }
                    match result {
                        Ok(directory) => self.dir_entries = directory,
                        Err(error) => self.status_notice = Some((error, true)),
                    }
                }
                // Si el usuario abrió otro archivo o cambió de rama, no se toca su editor.
                AsyncEvent::FileLoaded(key, result) => {
                    if !self.is_current_repo_request(&self.file_request, &key)
                        || self.current_branch != key.branch
                    {
                        continue;
                    }
                    match result {
                        Ok((content, sha)) => {
                            self.opened_file_path = Some(key.path);
                            self.opened_file_content = content;
                            self.opened_file_sha = sha;
                        }
                        Err(error) => self.status_notice = Some((error, true)),
                    }
                }
                // El commit puede terminar después de que el usuario navegue a otro sitio.
                AsyncEvent::CommitDone(key, result) => match result {
                    Ok(sha) => {
                        if self.is_current_repo_request(&self.commit_request, &key)
                            && self.current_branch == key.branch
                            && self.opened_file_path.as_deref() == Some(key.path.as_str())
                        {
                            self.opened_file_sha = sha;
                            self.commit_message.clear();
                        }
                        self.status_notice = Some((
                            format!("Commit & Push terminado en {} ({})", key.repo, key.branch),
                            false,
                        ));
                    }
                    Err(error) => {
                        self.status_notice = Some((format!("{}: {}", key.repo, error), true))
                    }
                },
                // El nombre creado se obtiene de la solicitud original, no del cuadro editable.
                AsyncEvent::BranchCreated(key, result) => {
                    // Dos creaciones consecutivas pueden terminar en distinto orden;
                    // sólo la última solicitud debe seleccionar una rama en pantalla.
                    if !self.is_current_repo_request(&self.branch_create_request, &key) {
                        continue;
                    }
                    match result {
                        Ok(()) => {
                            self.select_created_branch(&key.path);
                            self.new_branch_name.clear();
                            self.refresh_branches(ctx.clone());
                            self.load_directory(ctx.clone());
                        }
                        Err(error) => {
                            self.status_notice = Some((format!("{}: {}", key.repo, error), true))
                        }
                    }
                }
                AsyncEvent::ManagementLoaded(key, result) => {
                    if !self.is_current_repo_request(&self.management_request, &key) {
                        continue;
                    }
                    match result {
                        Ok(data) => self.mgmt_data = data,
                        Err(error) => self.status_notice = Some((error, true)),
                    }
                }
                // Una acción terminada para otro repo no recarga los datos del repo actual.
                AsyncEvent::MgmtActionDone(repo, result) => match result {
                    Ok(()) => {
                        if self
                            .selected_repo
                            .as_ref()
                            .map(|selected| selected.full_name.as_str())
                            == Some(repo.as_str())
                        {
                            self.load_management(ctx.clone());
                        }
                    }
                    Err(error) => self.status_notice = Some((format!("{}: {}", repo, error), true)),
                },
                // Las consultas Git locales no dependen del token ni de la sesión remota.
                #[cfg(not(target_arch = "wasm32"))]
                AsyncEvent::LocalRepositoryLoaded(generation, result) => {
                    if generation != self.local_generation {
                        continue;
                    }
                    self.local_loading = false;
                    match result {
                        Ok(repository) => {
                            self.local_repo_input = repository.root.to_string_lossy().into_owned();
                            self.local_repository = Some(repository);
                            self.local_error = None;
                        }
                        Err(error) => {
                            self.local_repository = None;
                            self.local_error = Some(error);
                        }
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                AsyncEvent::LocalDiffLoaded(generation, result) => {
                    if generation != self.local_diff_generation {
                        continue;
                    }
                    self.local_diff_loading = false;
                    match result {
                        Ok(diff) => self.local_diff_text = diff,
                        Err(error) => self.local_error = Some(error),
                    }
                }
                // E2: sólo el último preview de la carpeta activa puede mostrarse.
                #[cfg(not(target_arch = "wasm32"))]
                AsyncEvent::LocalActionPreviewLoaded(generation, result) => {
                    if generation != self.local_action_generation {
                        continue;
                    }
                    self.local_action_loading = false;
                    match result {
                        Ok(preview) => {
                            if self.local_repository.as_ref().map(|repo| &repo.root)
                                == Some(&preview.root)
                            {
                                self.local_action_preview = Some(preview);
                            }
                        }
                        Err(error) => self.local_error = Some(error),
                    }
                }
                // Tras una acción confirmada se recargan los datos ya devueltos.
                // También se invalidan diffs anteriores al nuevo índice.
                #[cfg(not(target_arch = "wasm32"))]
                AsyncEvent::LocalActionFinished(generation, action, result) => {
                    if generation != self.local_action_generation {
                        continue;
                    }
                    self.local_action_loading = false;
                    self.local_action_preview = None;
                    match result {
                        Ok(repository) => {
                            self.local_diff_generation = self.local_diff_generation.wrapping_add(1);
                            self.local_diff_loading = false;
                            self.local_diff_target = None;
                            self.local_diff_text.clear();
                            // El historial E3-A anterior podría ser distinto después
                            // de stage, unstage o commit: se descarta y debe recargarse.
                            self.history.invalidate();
                            self.local_repository = Some(repository);
                            self.local_action_notice = Some(match action {
                                LocalAction::Stage(path) => {
                                    format!(
                                        "{}: {}",
                                        self.i18n.t(self.language, "e2_notice_staged"),
                                        path
                                    )
                                }
                                LocalAction::Unstage(path) => {
                                    format!(
                                        "{}: {}",
                                        self.i18n.t(self.language, "e2_notice_unstaged"),
                                        path
                                    )
                                }
                                LocalAction::Commit(_) => {
                                    self.local_commit_message.clear();
                                    self.i18n
                                        .t(self.language, "e2_notice_committed")
                                        .to_string()
                                }
                            });
                            self.local_error = None;
                        }
                        Err(error) => self.local_error = Some(error),
                    }
                }
                // E3-A: aceptar sólo la instantánea solicitada más reciente.
                #[cfg(not(target_arch = "wasm32"))]
                AsyncEvent::HistoryLoaded(generation, result) => {
                    if generation != self.history.overview_generation {
                        continue;
                    }
                    self.history.loading = false;
                    match result {
                        Ok(overview) => {
                            if self.local_repository.as_ref().map(|repo| &repo.root)
                                == Some(&overview.root)
                            {
                                self.history.accept_overview(overview);
                            }
                        }
                        Err(error) => self.history.error = Some(error),
                    }
                }
                // E3-A: al abrir otra carpeta o efectuar un cambio E2,
                // las generaciones se incrementan y este resultado se descarta.
                #[cfg(not(target_arch = "wasm32"))]
                AsyncEvent::HistoryDetailLoaded(generation, result) => {
                    if generation != self.history.detail_generation {
                        continue;
                    }
                    self.history.detail_loading = false;
                    match result {
                        Ok(text) => self.history.detail = text,
                        Err(error) => self.history.error = Some(error),
                    }
                }
                // E3-B: sólo se muestra una vista previa válida para esta
                // carpeta y para la petición más reciente.
                #[cfg(not(target_arch = "wasm32"))]
                AsyncEvent::RefPreviewLoaded(generation, result) => {
                    if generation != self.history.ref_generation {
                        continue;
                    }
                    self.history.ref_loading = false;
                    match result {
                        Ok(preview) => {
                            if self.local_repository.as_ref().map(|repo| &repo.root)
                                == Some(&preview.root)
                            {
                                self.history.ref_preview = Some(preview);
                            }
                        }
                        Err(error) => self.history.ref_error = Some(error),
                    }
                }
                // E3-B: el comando Git ya ha terminado. Refrescar la vista
                // es un paso independiente, no un requisito para considerar
                // que una referencia se creó correctamente.
                #[cfg(not(target_arch = "wasm32"))]
                AsyncEvent::RefCreationFinished(generation, result) => {
                    if generation != self.history.ref_generation {
                        continue;
                    }
                    self.history.ref_loading = false;
                    self.history.ref_preview = None;
                    match result {
                        Ok(created) => {
                            if self.local_repository.as_ref().map(|repo| &repo.root)
                                == Some(&created.root)
                            {
                                let label_key = match created.action.kind {
                                    RefKind::Branch => "e3b_branch_created",
                                    RefKind::Tag => "e3b_tag_created",
                                };
                                let notice = format!(
                                    "{}: {}",
                                    self.i18n.t(self.language, label_key),
                                    created.action.name
                                );
                                // Releer E1 e invalidar la fotografía E3-A.
                                // La rama activa y los archivos no cambian.
                                self.refresh_local_repository(ctx.clone());
                                self.local_action_notice = Some(notice);
                            }
                        }
                        Err(error) => self.history.ref_error = Some(error),
                    }
                }
                AsyncEvent::McpToolsLoaded(Ok(tools)) => {
                    if let Some(first) = tools.first() {
                        self.mcp_selected_tool = first.name.clone();
                    }
                    self.mcp_tools = tools;
                }
                AsyncEvent::McpCallResult(Ok(out)) => {
                    self.mcp_last_output = out;
                }
                AsyncEvent::McpToolsLoaded(Err(error)) | AsyncEvent::McpCallResult(Err(error)) => {
                    self.status_notice = Some((error, true));
                }
            }
        }

        // El nuevo shell conserva las peticiones Git/GitHub anteriores.
        // Sólo cambia cómo se presentan las opciones en la ventana.
        render_app_shell(self, ctx);
    }
}

// Pruebas de seguridad sin acceso a GitHub ni ejecución de programas externos.
#[cfg(test)]
mod security_tests {
    use super::*;

    /// Comprueba que el token en memoria no se escribe en el JSON de preferencias.
    #[test]
    fn github_token_is_not_persisted() {
        let mut app = GitCraftApp::default();
        app.saved_token = "token_de_prueba_no_real".into();
        let preferences = serde_json::to_value(&app).expect("el estado debe poder serializarse");
        assert!(preferences.get("saved_token").is_none());
        // La carga de preferencias antiguas ignora un token que antes pudiera persistirse.
        let old = serde_json::json!({"saved_token":"secreto_antiguo"});
        let reloaded: GitCraftApp =
            serde_json::from_value(old).expect("configuración antigua válida");
        assert!(reloaded.saved_token.is_empty());
    }

    /// Una respuesta de otro repositorio o de una petición antigua se descarta.
    #[test]
    fn stale_repo_responses_are_rejected() {
        let mut app = GitCraftApp::default();
        app.selected_repo = Some(GitHubRepo {
            full_name: "Samafra/gitcraft".into(),
            ..Default::default()
        });
        let old = app.new_repo_request("Samafra/gitcraft", "main", "src");
        app.directory_request = Some(old.clone());
        assert!(app.is_current_repo_request(&app.directory_request, &old));
        let newest = app.new_repo_request("Samafra/gitcraft", "main", "src");
        app.directory_request = Some(newest);
        assert!(!app.is_current_repo_request(&app.directory_request, &old));
        app.selected_repo = Some(GitHubRepo {
            full_name: "Otra/carpeta".into(),
            ..Default::default()
        });
        assert!(!app.is_current_repo_request(&app.directory_request, &old));
    }

    /// Tras seleccionar la rama recién creada, no se vuelve a la rama por defecto.
    #[test]
    fn created_branch_keeps_the_new_selection() {
        let mut app = GitCraftApp::default();
        app.select_created_branch("feature/prueba");
        assert_eq!(app.current_branch, "feature/prueba");
        assert!(app.opened_file_path.is_none());
    }

    /// Desconectar anula el permiso MCP y las respuestas de login aún pendientes.
    #[test]
    fn disconnect_invalidates_pending_session_and_token_permissions() {
        let mut app = GitCraftApp::default();
        app.saved_token = "token_solo_prueba".into();
        app.mcp_token_authorized_endpoint = Some(app.mcp_endpoint.clone());
        let before = app.auth_generation;
        app.disconnect_github();
        assert!(app.saved_token.is_empty());
        assert!(app.mcp_token_authorized_endpoint.is_none());
        assert_ne!(app.auth_generation, before);
    }

    /// Nadie recibe un PAT de forma implícita, ni siquiera el servidor MCP por defecto.
    #[test]
    fn mcp_token_requires_explicit_consent_for_exact_endpoint() {
        let mut app = GitCraftApp::default();
        app.saved_token = "token_de_prueba_no_real".into();
        assert!(app.token_for_mcp().is_empty());
        app.mcp_token_authorized_endpoint = Some(app.mcp_endpoint.clone());
        assert_eq!(app.token_for_mcp(), app.saved_token);
        app.mcp_endpoint = "otro-programa-mcp".into();
        assert!(app.token_for_mcp().is_empty());
        app.mcp_endpoint = "npx -y @modelcontextprotocol/server-github".into();
        app.mcp_transport = McpTransport::Http;
        assert!(app.token_for_mcp().is_empty());
    }
}
