use crate::github::{fetch_user_and_repos, GitHubRepo, GitHubUser};
use crate::github_manage::{close_issue, create_issue, create_pr, create_release, fetch_management_data, merge_pr, rerun_workflow, RepoManagementData};
use crate::i18n::{I18nManager, Language};
use crate::repo_ops::{commit_file_push, create_branch, fetch_branches, fetch_directory, fetch_file_content, GitBranch, RepoEntry};
use crate::mcp_client::{mcp_call_tool, mcp_discover_tools, McpTool, McpTransport};
use crate::ui_changes::render_changes_tab;
use crate::ui_mcp::render_mcp_ui;
use crate::ui_manage::{render_actions_tab, render_issues_tab, render_prs_tab, render_releases_tab};
use crate::ui_repos::render_repos_tab;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveTab { Repositories, Changes, PullRequests, Issues, Actions, Releases, Mcp }

pub enum AsyncEvent {
    UserAndRepos(Result<(GitHubUser, Vec<GitHubRepo>), String>),
    Branches(Result<Vec<GitBranch>, String>),
    Directory(Result<Vec<RepoEntry>, String>),
    FileLoaded(String, Result<(String, String), String>),
    CommitDone(Result<String, String>),
    BranchCreated(Result<(), String>),
    ManagementLoaded(Result<RepoManagementData, String>),
    MgmtActionDone(Result<(), String>),
    McpToolsLoaded(Result<Vec<McpTool>, String>),
    McpCallResult(Result<String, String>),
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct GitCraftApp {
    pub language: Language,
    pub active_tab: ActiveTab,
    pub dark_mode: bool,
    pub show_mcp_sidebar: bool,
    pub saved_token: String,
    #[serde(skip)] pub i18n: I18nManager,
    #[serde(skip)] pub is_loading: bool,
    #[serde(skip)] pub status_notice: Option<(String, bool)>,
    #[serde(skip)] pub user: Option<GitHubUser>,
    #[serde(skip)] pub repos: Vec<GitHubRepo>,
    #[serde(skip)] pub selected_repo: Option<GitHubRepo>,
    #[serde(skip)] pub repo_filter: String,
    #[serde(skip)] pub branches: Vec<GitBranch>,
    #[serde(skip)] pub current_branch: String,
    #[serde(skip)] pub new_branch_name: String,
    #[serde(skip)] pub current_path: String,
    #[serde(skip)] pub dir_entries: Vec<RepoEntry>,
    #[serde(skip)] pub opened_file_path: Option<String>,
    #[serde(skip)] pub opened_file_sha: String,
    #[serde(skip)] pub opened_file_content: String,
    #[serde(skip)] pub commit_message: String,
    #[serde(skip)] pub mgmt_data: RepoManagementData,
    #[serde(skip)] pub new_pr_title: String,
    #[serde(skip)] pub new_pr_head: String,
    #[serde(skip)] pub new_pr_base: String,
    #[serde(skip)] pub new_issue_title: String,
    #[serde(skip)] pub new_issue_body: String,
    #[serde(skip)] pub new_rel_tag: String,
    #[serde(skip)] pub new_rel_name: String,
    #[serde(skip)] pub new_rel_body: String,
    pub mcp_transport: McpTransport,
    pub mcp_endpoint: String,
    #[serde(skip)] pub mcp_tools: Vec<McpTool>,
    #[serde(skip)] pub mcp_selected_tool: String,
    #[serde(skip)] pub mcp_tool_args: String,
    #[serde(skip)] pub mcp_last_output: String,
    #[serde(skip)] tx: Option<Sender<AsyncEvent>>,
    #[serde(skip)] rx: Option<Receiver<AsyncEvent>>,
}

impl Default for GitCraftApp {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            language: Language::Es, active_tab: ActiveTab::Repositories, dark_mode: true, show_mcp_sidebar: true,
            saved_token: String::new(), i18n: I18nManager::default(), is_loading: false, status_notice: None,
            user: None, repos: Vec::new(), selected_repo: None, repo_filter: String::new(),
            branches: Vec::new(), current_branch: "main".into(), new_branch_name: String::new(),
            current_path: String::new(), dir_entries: Vec::new(), opened_file_path: None,
            opened_file_sha: String::new(), opened_file_content: String::new(), commit_message: String::new(),
            mgmt_data: RepoManagementData::default(), new_pr_title: String::new(), new_pr_head: String::new(), new_pr_base: "main".into(),
            new_issue_title: String::new(), new_issue_body: String::new(), new_rel_tag: String::new(), new_rel_name: String::new(), new_rel_body: String::new(),
            mcp_transport: McpTransport::Stdio, mcp_endpoint: "npx -y @modelcontextprotocol/server-github".into(),
            mcp_tools: Vec::new(), mcp_selected_tool: String::new(), mcp_tool_args: "{}".into(), mcp_last_output: String::new(),
            tx: Some(tx), rx: Some(rx),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_task<F>(ctx: egui::Context, fut: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    std::thread::spawn(move || { tokio::runtime::Runtime::new().unwrap().block_on(fut); ctx.request_repaint(); });
}

#[cfg(target_arch = "wasm32")]
fn spawn_task<F>(ctx: egui::Context, fut: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(async move { fut.await; ctx.request_repaint(); });
}

impl GitCraftApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app: Self = cc.storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY)).unwrap_or_default();
        let (tx, rx) = channel(); app.tx = Some(tx); app.rx = Some(rx); app.apply_theme(&cc.egui_ctx);
        if !app.saved_token.trim().is_empty() { app.trigger_github_sync(cc.egui_ctx.clone()); }
        app
    }
    fn apply_theme(&self, ctx: &egui::Context) { ctx.set_visuals(if self.dark_mode { egui::Visuals::dark() } else { egui::Visuals::light() }); }
    pub fn trigger_github_sync(&mut self, ctx: egui::Context) {
        self.is_loading = true; let (t, tx) = (self.saved_token.clone(), self.tx.clone().unwrap());
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::UserAndRepos(fetch_user_and_repos(t).await)); });
    }
    pub fn load_branches_and_root(&mut self, ctx: egui::Context) {
        let Some(r) = &self.selected_repo else { return };
        self.current_branch = r.default_branch.clone(); self.new_pr_base = r.default_branch.clone();
        self.current_path.clear(); self.opened_file_path = None;
        let (t, f, tx) = (self.saved_token.clone(), r.full_name.clone(), self.tx.clone().unwrap());
        spawn_task(ctx.clone(), async move { let _ = tx.send(AsyncEvent::Branches(fetch_branches(t, f).await)); });
        self.load_directory(ctx.clone()); self.load_management(ctx);
    }
    pub fn load_directory(&mut self, ctx: egui::Context) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, b, p, tx) = (self.saved_token.clone(), r.full_name.clone(), self.current_branch.clone(), self.current_path.clone(), self.tx.clone().unwrap());
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::Directory(fetch_directory(t, f, b, p).await)); });
    }
    pub fn open_file(&mut self, ctx: egui::Context, path: String) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, b, tx, pc) = (self.saved_token.clone(), r.full_name.clone(), self.current_branch.clone(), self.tx.clone().unwrap(), path.clone());
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::FileLoaded(pc, fetch_file_content(t, f, b, path).await)); });
    }
    pub fn trigger_create_branch(&mut self, ctx: egui::Context) {
        let Some(r) = &self.selected_repo else { return };
        let sha = self.branches.iter().find(|b| b.name == self.current_branch).map(|b| b.commit.sha.clone()).unwrap_or_default();
        let (t, f, nb, tx) = (self.saved_token.clone(), r.full_name.clone(), self.new_branch_name.clone(), self.tx.clone().unwrap());
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::BranchCreated(create_branch(t, f, nb, sha).await)); });
    }
    pub fn trigger_commit_push(&mut self, ctx: egui::Context) {
        let (Some(r), Some(p)) = (&self.selected_repo, &self.opened_file_path) else { return };
        let msg = if self.commit_message.trim().is_empty() { format!("Update {}", p) } else { self.commit_message.clone() };
        let (t, f, b, p, s, c, tx) = (self.saved_token.clone(), r.full_name.clone(), self.current_branch.clone(), p.clone(), self.opened_file_sha.clone(), self.opened_file_content.clone(), self.tx.clone().unwrap());
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::CommitDone(commit_file_push(t, f, b, p, s, c, msg).await)); });
    }
    pub fn load_management(&mut self, ctx: egui::Context) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, tx) = (self.saved_token.clone(), r.full_name.clone(), self.tx.clone().unwrap());
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::ManagementLoaded(fetch_management_data(t, f).await)); });
    }
    pub fn trigger_create_pr(&mut self, ctx: egui::Context) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, ti, h, b, tx) = (self.saved_token.clone(), r.full_name.clone(), self.new_pr_title.clone(), self.new_pr_head.clone(), self.new_pr_base.clone(), self.tx.clone().unwrap());
        self.new_pr_title.clear();
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::MgmtActionDone(create_pr(t, f, ti, h, b).await)); });
    }
    pub fn trigger_merge_pr(&mut self, ctx: egui::Context, num: u64) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, tx) = (self.saved_token.clone(), r.full_name.clone(), self.tx.clone().unwrap());
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::MgmtActionDone(merge_pr(t, f, num).await)); });
    }
    pub fn trigger_create_issue(&mut self, ctx: egui::Context) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, ti, bo, tx) = (self.saved_token.clone(), r.full_name.clone(), self.new_issue_title.clone(), self.new_issue_body.clone(), self.tx.clone().unwrap());
        self.new_issue_title.clear(); self.new_issue_body.clear();
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::MgmtActionDone(create_issue(t, f, ti, bo).await)); });
    }
    pub fn trigger_close_issue(&mut self, ctx: egui::Context, num: u64) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, tx) = (self.saved_token.clone(), r.full_name.clone(), self.tx.clone().unwrap());
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::MgmtActionDone(close_issue(t, f, num).await)); });
    }
    pub fn trigger_rerun_workflow(&mut self, ctx: egui::Context, id: u64) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, tx) = (self.saved_token.clone(), r.full_name.clone(), self.tx.clone().unwrap());
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::MgmtActionDone(rerun_workflow(t, f, id).await)); });
    }
    pub fn trigger_mcp_discover(&mut self, ctx: egui::Context) {
        self.is_loading = true;
        let (tr, ep, tok, tx) = (self.mcp_transport, self.mcp_endpoint.clone(), self.saved_token.clone(), self.tx.clone().unwrap());
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::McpToolsLoaded(mcp_discover_tools(tr, ep, tok).await)); });
    }
    pub fn trigger_mcp_call(&mut self, ctx: egui::Context) {
        self.is_loading = true;
        let (tr, ep, tok, tool, args, tx) = (self.mcp_transport, self.mcp_endpoint.clone(), self.saved_token.clone(), self.mcp_selected_tool.clone(), self.mcp_tool_args.clone(), self.tx.clone().unwrap());
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::McpCallResult(mcp_call_tool(tr, ep, tok, tool, args).await)); });
    }
    pub fn trigger_create_release(&mut self, ctx: egui::Context) {
        let Some(r) = &self.selected_repo else { return };
        let (t, f, tg, nm, bo, tx) = (self.saved_token.clone(), r.full_name.clone(), self.new_rel_tag.clone(), self.new_rel_name.clone(), self.new_rel_body.clone(), self.tx.clone().unwrap());
        self.new_rel_tag.clear(); self.new_rel_name.clear(); self.new_rel_body.clear();
        spawn_task(ctx, async move { let _ = tx.send(AsyncEvent::MgmtActionDone(create_release(t, f, tg, nm, bo).await)); });
    }
}

impl eframe::App for GitCraftApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) { eframe::set_value(storage, eframe::APP_KEY, self); }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut pending = Vec::new();
        if let Some(rx) = &self.rx { while let Ok(ev) = rx.try_recv() { pending.push(ev); } }
        for ev in pending {
            self.is_loading = false;
            match ev {
                AsyncEvent::UserAndRepos(Ok((u, r))) => { self.user = Some(u); self.repos = r; self.status_notice = None; }
                AsyncEvent::UserAndRepos(Err(e)) | AsyncEvent::Branches(Err(e)) | AsyncEvent::Directory(Err(e))

                | AsyncEvent::FileLoaded(_, Err(e)) | AsyncEvent::CommitDone(Err(e)) | AsyncEvent::BranchCreated(Err(e))
                | AsyncEvent::ManagementLoaded(Err(e)) | AsyncEvent::MgmtActionDone(Err(e)) => { self.status_notice = Some((e, true)); }
                AsyncEvent::Branches(Ok(b)) => { self.branches = b; }
                AsyncEvent::Directory(Ok(d)) => { self.dir_entries = d; }
                AsyncEvent::FileLoaded(p, Ok((txt, sha))) => { self.opened_file_path = Some(p); self.opened_file_content = txt; self.opened_file_sha = sha; }
                AsyncEvent::CommitDone(Ok(s)) => { self.opened_file_sha = s; self.commit_message.clear(); self.status_notice = Some((self.i18n.t(self.language, "commit_success").into(), false)); }
                AsyncEvent::BranchCreated(Ok(())) => { self.current_branch = self.new_branch_name.trim().to_string(); self.new_branch_name.clear(); self.load_branches_and_root(ctx.clone()); }
                AsyncEvent::ManagementLoaded(Ok(d)) => { self.mgmt_data = d; }
                AsyncEvent::MgmtActionDone(Ok(())) => { self.load_management(ctx.clone()); }
                AsyncEvent::McpToolsLoaded(Ok(tools)) => { if let Some(first) = tools.first() { self.mcp_selected_tool = first.name.clone(); } self.mcp_tools = tools; }
                AsyncEvent::McpCallResult(Ok(out)) => { self.mcp_last_output = out; }
                AsyncEvent::McpToolsLoaded(Err(e)) | AsyncEvent::McpCallResult(Err(e)) => { self.status_notice = Some((e, true)); }
            }
        }

        let lang = self.language;
        egui::TopBottomPanel::top("top_nav").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🦀 GitCraft");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_salt("lang").selected_text(lang.label()).show_ui(ui, |ui| {
                        for l in Language::all() { ui.selectable_value(&mut self.language, *l, l.label()); }
                    });
                    if ui.button(if self.dark_mode { "☀" } else { "🌙" }).clicked() { self.dark_mode = !self.dark_mode; self.apply_theme(ctx); }
                    if ui.selectable_label(self.show_mcp_sidebar, "🤖 MCP").clicked() { self.show_mcp_sidebar = !self.show_mcp_sidebar; }
                });
            });
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.active_tab, ActiveTab::Repositories, self.i18n.t(lang, "tab_repos"));
                ui.selectable_value(&mut self.active_tab, ActiveTab::Changes, self.i18n.t(lang, "tab_changes"));
                ui.selectable_value(&mut self.active_tab, ActiveTab::PullRequests, self.i18n.t(lang, "tab_prs"));
                ui.selectable_value(&mut self.active_tab, ActiveTab::Issues, self.i18n.t(lang, "tab_issues"));
                ui.selectable_value(&mut self.active_tab, ActiveTab::Actions, self.i18n.t(lang, "tab_actions"));
                ui.selectable_value(&mut self.active_tab, ActiveTab::Releases, self.i18n.t(lang, "tab_releases"));
                ui.selectable_value(&mut self.active_tab, ActiveTab::Mcp, self.i18n.t(lang, "tab_mcp"));
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(r) = &self.selected_repo { ui.label(format!("{} {} ({})", self.i18n.t(lang, "active_repo_label"), r.full_name, self.current_branch)); }
                if let Some((msg, is_err)) = &self.status_notice { ui.colored_label(if *is_err { egui::Color32::LIGHT_RED } else { egui::Color32::LIGHT_GREEN }, msg); }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(u) = &self.user { ui.colored_label(egui::Color32::LIGHT_GREEN, format!("@{} ", u.login)); }
                });
            });
        });

        if self.show_mcp_sidebar {
            egui::SidePanel::right("mcp_panel").default_width(250.0).show(ctx, |ui| {
                render_mcp_ui(self, ui, ctx);
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.user.is_none() {
                ui.heading(self.i18n.t(lang, "welcome_title"));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.saved_token).password(true).hint_text("ghp_xxxx"));
                    if ui.button(self.i18n.t(lang, "auth_connect_btn")).clicked() { self.trigger_github_sync(ctx.clone()); }
                });
                return;
            }
            match self.active_tab {
                ActiveTab::Repositories => render_repos_tab(self, ui, ctx),
                ActiveTab::Changes => render_changes_tab(self, ui, ctx),
                ActiveTab::PullRequests => render_prs_tab(self, ui, ctx),
                ActiveTab::Issues => render_issues_tab(self, ui, ctx),
                ActiveTab::Actions => render_actions_tab(self, ui, ctx),
                ActiveTab::Releases => render_releases_tab(self, ui, ctx),
                ActiveTab::Mcp => render_mcp_ui(self, ui, ctx),
            }
        });
    }
}
