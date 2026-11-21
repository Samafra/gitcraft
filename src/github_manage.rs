//! Funciones de gestión remota en GitHub: Pull Requests, Issues, Actions y Releases.
//!
//! Todas estas operaciones usan HTTP, no modifican directamente un repositorio Git local.
//! Los fallos HTTP deben propagarse y no confundirse con una lista vacía.

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

/// Lee una colección de la API GitHub y valida antes el código HTTP.
///
/// Devuelve `Err` para errores como 403 (sin permisos) y 429 (límite alcanzado):
/// de lo contrario, una pantalla podría confundir un fallo con una lista vacía.
/// `T` representa el tipo de respuesta JSON esperado (por ejemplo, `Vec<IssueItem>`).
async fn fetch_collection<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: String,
    resource: &str,
) -> Result<T, String> {
    // Primero terminamos la petición; un error de red se comunica al usuario.
    let response = client.get(url).send().await
        .map_err(|error| format!("Fallo de red al consultar {}: {}", resource, error))?;
    let status = response.status();
    if !status.is_success() {
        // Limitar la longitud de la respuesta para no llenar el panel de errores.
        let details = response.text().await.unwrap_or_default();
        let details: String = details.chars().take(240).collect();
        return Err(format!("GitHub respondió {} al consultar {}: {}", status, resource, details));
    }
    // Un HTTP 200 con JSON inesperado también es un error, no una lista vacía.
    response.json::<T>().await
        .map_err(|error| format!("Respuesta JSON no válida para {}: {}", resource, error))
}

// Descarga PRs, Issues, Actions y Releases del repositorio seleccionado.
/// Recupera las colecciones de gestión para un repositorio concreto.
/// Ante cualquier respuesta HTTP o JSON errónea devuelve `Err`, nunca datos inventados.
pub async fn fetch_management_data(token: String, full_name: String) -> Result<RepoManagementData, String> {
    let (client, api) = client_and_base(&token)?;

    // La API GitHub tiene endpoints independientes para cada colección. Cualquier
    // error se devuelve como `Err`; nunca se disfraza de lista vacía.
    let prs: Vec<PullRequestItem> = fetch_collection(
        &client,
        format!("{}/repos/{}/pulls?state=open&per_page=50", api, full_name),
        "Pull Requests",
    ).await?;

    let all_issues: Vec<IssueItem> = fetch_collection(
        &client,
        format!("{}/repos/{}/issues?state=open&per_page=50", api, full_name),
        "Issues",
    ).await?;
    // GitHub incluye PRs en el endpoint de Issues: los excluimos de esa pestaña.
    let issues = all_issues.into_iter().filter(|item| item.pull_request.is_none()).collect();

    let runs_wrap: WorkflowRunsWrapper = fetch_collection(
        &client,
        format!("{}/repos/{}/actions/runs?per_page=20", api, full_name),
        "GitHub Actions",
    ).await?;

    let releases: Vec<ReleaseItem> = fetch_collection(
        &client,
        format!("{}/repos/{}/releases?per_page=20", api, full_name),
        "Releases",
    ).await?;

    Ok(RepoManagementData { prs, issues, runs: runs_wrap.workflow_runs, releases })
}

// Crea un nuevo Pull Request.
/// Abre un Pull Request entre dos ramas del repositorio indicado.
pub async fn create_pr(token: String, full_name: String, title: String, head: String, base: String) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.post(format!("{}/repos/{}/pulls", api, full_name))
        .json(&json!({ "title": title, "head": head, "base": base }))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}

// Fusiona (Merge) un Pull Request abierto.
/// Fusiona un Pull Request mediante GitHub, con los permisos del usuario autenticado.
pub async fn merge_pr(token: String, full_name: String, pr_number: u64) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.put(format!("{}/repos/{}/pulls/{}/merge", api, full_name, pr_number))
        .json(&json!({ "merge_method": "merge" }))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}

// Crea un nuevo Issue.
/// Crea un Issue con título y contenido en el repositorio remoto.
pub async fn create_issue(token: String, full_name: String, title: String, body: String) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.post(format!("{}/repos/{}/issues", api, full_name))
        .json(&json!({ "title": title, "body": body }))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}

// Cierra un Issue abierto.
/// Cambia un Issue a cerrado, preservando sus mensajes y comentarios.
pub async fn close_issue(token: String, full_name: String, issue_number: u64) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.patch(format!("{}/repos/{}/issues/{}", api, full_name, issue_number))
        .json(&json!({ "state": "closed" }))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}

// Relanza una ejecución de GitHub Actions.
/// Solicita a GitHub relanzar un workflow existente; no es el mecanismo de validación de GitCraft.
pub async fn rerun_workflow(token: String, full_name: String, run_id: u64) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.post(format!("{}/repos/{}/actions/runs/{}/rerun", api, full_name, run_id))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}

// Publica una nueva Release con su tag.
/// Publica una Release remota con etiqueta, título y notas.
pub async fn create_release(token: String, full_name: String, tag: String, name: String, body: String) -> Result<(), String> {
    let (client, api) = client_and_base(&token)?;
    let resp = client.post(format!("{}/repos/{}/releases", api, full_name))
        .json(&json!({ "tag_name": tag, "name": name, "body": body }))
        .send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(resp.text().await.unwrap_or_default()); }
    Ok(())
}

// Pruebas de respuesta HTTP contra un servidor de prueba local, nunca contra GitHub.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Envía una respuesta HTTP controlada para poder simular errores de GitHub.
    /// El servidor dura una sola petición y no modifica repositorios reales.
    fn start_test_server(status: &str, body: &str) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("puerto de pruebas");
        let address = listener.local_addr().expect("dirección de pruebas");
        let response = format!(
            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            body.len(),
            body,
        );
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("petición de prueba");
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer);
            stream.write_all(response.as_bytes()).expect("respuesta de prueba");
        });
        (format!("http://{}", address), handle)
    }

    /// Un HTTP 403 no debe convertirse en una colección vacía de Issues.
    #[tokio::test]
    async fn permission_denied_is_reported() {
        let (url, server) = start_test_server("403 Forbidden", r#"{"message":"Forbidden"}"#);
        let result: Result<Vec<IssueItem>, String> =
            fetch_collection(&reqwest::Client::new(), url, "Issues").await;
        server.join().expect("servidor de pruebas finalizado");
        let error = result.expect_err("403 debe generar error");
        assert!(error.contains("403"));
        assert!(error.contains("Issues"));
    }

    /// Una petición limitada por GitHub tampoco debe mostrarse sin resultados.
    #[tokio::test]
    async fn rate_limit_is_reported() {
        let (url, server) = start_test_server("429 Too Many Requests", r#"{"message":"Rate limit"}"#);
        let result: Result<Vec<IssueItem>, String> =
            fetch_collection(&reqwest::Client::new(), url, "Issues").await;
        server.join().expect("servidor de pruebas finalizado");
        assert!(result.expect_err("429 debe generar error").contains("429"));
    }
}
