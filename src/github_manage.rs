use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::json;

// Estructura para representar un Pull Request de GitHub.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PullRequestItem {
    pub number: u64,
    pub title: String,
    pub state: String, // "open" o "closed"
    pub html_url: String,
}

// Estructura para representar un Issue de GitHub.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IssueItem {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub pull_request: Option<serde_json::Value>, // GitHub devuelve PRs dentro de /issues; si es Some, lo filtramos.
}

// Representa el usuario que disparó la ejecución del workflow.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkflowActor {
    pub login: String,
}

// Estructura para representar una ejecución de GitHub Actions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkflowRunItem {
    pub id: u64,
    pub name: String,
    pub status: String,             // "completed", "in_progress", "queued"
    pub conclusion: Option<String>, // Some("success"), Some("failure"), None
    pub head_branch: String,
    pub created_at: String,         // Fecha de ejecución ISO 8601
    pub actor: WorkflowActor,       // Usuario que ejecutó el workflow
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct WorkflowRunsWrapper {
    pub workflow_runs: Vec<WorkflowRunItem>,
}

// Estructura para representar una Release publicada.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReleaseItem {
    pub id: u64,
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
}

// Contenedor con todos los datos de gestión de un repositorio cargados de una vez.
#[derive(Debug, Clone, Default)]
pub struct RepoManagementData {
    pub prs: Vec<PullRequestItem>,
    pub issues: Vec<IssueItem>,
    pub runs: Vec<WorkflowRunItem>,
    pub releases: Vec<ReleaseItem>,
}

// Función interna para preparar el cliente HTTP con cabeceras de autenticación.
fn client_and_base(token: &str) -> Result<(reqwest::Client, String), String> {
    let api = ["https://", "api.", "github.com"].concat();
    let mut headers = HeaderMap::new();
    let auth = HeaderValue::from_str(&format!("Bearer {}", token.trim())).map_err(|_| "Token inválido".to_string())?;
    headers.insert(AUTHORIZATION, auth);
    headers.insert(USER_AGENT, HeaderValue::from_static("GitCraft-Rust-Client"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
    headers.insert("X-GitHub-Api-Version", HeaderValue::from_static("2022-11-28"));
    let client = reqwest::Client::builder().default_headers(headers).build().map_err(|e| e.to_string())?;
    Ok((client, api))
}

// Descarga PRs, Issues, Actions y Releases del repositorio seleccionado.
pub async fn fetch_management_data(token: String, full_name: String) -> Result<RepoManagementData, String> {
    let (client, api) = client_and_base(&token)?;

    let prs: Vec<PullRequestItem> = client
        .get(format!("{}/repos/{}/pulls?state=open&per_page=50", api, full_name))
        .send().await.map_err(|e| e.to_string())?.json().await.unwrap_or_default();

    let all_issues: Vec<IssueItem> = client
        .get(format!("{}/repos/{}/issues?state=open&per_page=50", api, full_name))
        .send().await.map_err(|e| e.to_string())?.json().await.unwrap_or_default();
    // Filtramos solo Issues reales (los que no tienen el campo pull_request).
    let issues = all_issues.into_iter().filter(|i| i.pull_request.is_none()).collect();

    let runs_wrap: WorkflowRunsWrapper = client
        .get(format!("{}/repos/{}/actions/runs?per_page=20", api, full_name))
        .send().await.map_err(|e| e.to_string())?.json().await.unwrap_or_default();

    let releases: Vec<ReleaseItem> = client
        .get(format!("{}/repos/{}/releases?per_page=20", api, full_name))
        .send().await.map_err(|e| e.to_string())?.json().await.unwrap_or_default();

    Ok(RepoManagementData { prs, issues, runs: runs_wrap.workflow_runs, releases })
}

// Crea un nuevo Pull Request.
pub async fn create_pr(token: String, full_name: String, title: String, head: String, base: String) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.post(format!("{}/repos/{}/pulls", api, full_name))
        .json(&json!({ "title": title, "head": head, "base": base }))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}

// Fusiona (Merge) un Pull Request abierto.
pub async fn merge_pr(token: String, full_name: String, pr_number: u64) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.put(format!("{}/repos/{}/pulls/{}/merge", api, full_name, pr_number))
        .json(&json!({ "merge_method": "merge" }))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}

// Crea un nuevo Issue.
pub async fn create_issue(token: String, full_name: String, title: String, body: String) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.post(format!("{}/repos/{}/issues", api, full_name))
        .json(&json!({ "title": title, "body": body }))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}

// Cierra un Issue abierto.
pub async fn close_issue(token: String, full_name: String, issue_number: u64) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.patch(format!("{}/repos/{}/issues/{}", api, full_name, issue_number))
        .json(&json!({ "state": "closed" }))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}

// Relanza una ejecución de GitHub Actions.
pub async fn rerun_workflow(token: String, full_name: String, run_id: u64) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.post(format!("{}/repos/{}/actions/runs/{}/rerun", api, full_name, run_id))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}

// Publica una nueva Release con su tag.
pub async fn create_release(token: String, full_name: String, tag: String, name: String, body: String) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.post(format!("{}/repos/{}/releases", api, full_name))
        .json(&json!({ "tag_name": tag, "name": name, "body": body }))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}
