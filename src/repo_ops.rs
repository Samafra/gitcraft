use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GitBranch {
    pub name: String,
    pub commit: BranchCommit,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BranchCommit {
    pub sha: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RepoEntry {
    pub name: String,
    pub path: String,
    pub sha: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub content: Option<String>,
}

// Función auxiliar para codificar espacios y caracteres especiales en rutas/ramas de URL.
fn encode_url_segment(input: &str) -> String {
    input
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('#', "%23")
        .replace('?', "%3F")
}

fn build_client(token: &str) -> Result<(reqwest::Client, String), String> {
    let api_base = ["https://", "api.", "github.com"].concat();
    let mut headers = HeaderMap::new();
    let auth = HeaderValue::from_str(&format!("Bearer {}", token.trim()))
        .map_err(|_| "Token inválido".to_string())?;
    headers.insert(AUTHORIZATION, auth);
    headers.insert(USER_AGENT, HeaderValue::from_static("GitCraft-Rust-Client"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
    headers.insert("X-GitHub-Api-Version", HeaderValue::from_static("2022-11-28"));

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Error HTTP: {}", e))?;
    Ok((client, api_base))
}

pub async fn fetch_branches(token: String, full_name: String) -> Result<Vec<GitBranch>, String> {
    let (client, api) = build_client(&token)?;
    let url = format!("{}/repos/{}/branches?per_page=100", api, full_name);
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    // Si el repositorio está vacío, GitHub devuelve 404 o lista vacía; devolvemos Vec vacío sin error.
    if resp.status().as_u16() == 404 {
        return Ok(Vec::new());
    }
    if !resp.status().is_success() {
        return Err(format!("Error ramas ({})", resp.status()));
    }
    resp.json::<Vec<GitBranch>>().await.map_err(|e| e.to_string())
}

pub async fn create_branch(
    token: String,
    full_name: String,
    new_branch: String,
    base_sha: String,
) -> Result<(), String> {
    let (client, api) = build_client(&token)?;
    let url = format!("{}/repos/{}/git/refs", api, full_name);
    let body = json!({
        "ref": format!("refs/heads/{}", new_branch.trim()),
        "sha": base_sha
    });
    let resp = client.post(&url).json(&body).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!("No se pudo crear la rama: {}", txt));
    }
    Ok(())
}

pub async fn fetch_directory(
    token: String,
    full_name: String,
    branch: String,
    path: String,
) -> Result<Vec<RepoEntry>, String> {
    let (client, api) = build_client(&token)?;
    let clean_path = encode_url_segment(path.trim_start_matches('/'));
    let clean_branch = encode_url_segment(&branch);
    let url = if clean_path.is_empty() {
        format!("{}/repos/{}/contents?ref={}", api, full_name, clean_branch)
    } else {
        format!("{}/repos/{}/contents/{}?ref={}", api, full_name, clean_path, clean_branch)
    };

    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    // Si el repositorio está vacío (sin commits aún), devolvemos lista vacía sin mostrar error.
    if resp.status().as_u16() == 404 && path.is_empty() {
        return Ok(Vec::new());
    }
    if !resp.status().is_success() {
        return Err(format!("Error listando archivos ({}) en '{}'", resp.status(), path));
    }
    let mut entries = resp.json::<Vec<RepoEntry>>().await.map_err(|e| e.to_string())?;
    entries.sort_by(|a, b| b.entry_type.cmp(&a.entry_type).then(a.name.cmp(&b.name)));
    Ok(entries)
}

pub async fn fetch_file_content(
    token: String,
    full_name: String,
    branch: String,
    path: String,
) -> Result<(String, String), String> {
    let (client, api) = build_client(&token)?;
    let clean_path = encode_url_segment(path.trim_start_matches('/'));
    let clean_branch = encode_url_segment(&branch);
    let url = format!("{}/repos/{}/contents/{}?ref={}", api, full_name, clean_path, clean_branch);

    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Error abriendo archivo ({}) en '{}'", resp.status(), path));
    }
    let entry: RepoEntry = resp.json().await.map_err(|e| e.to_string())?;
    let raw_b64 = entry.content.unwrap_or_default().replace('\n', "");
    if raw_b64.is_empty() {
        return Ok(("[Archivo vacío o demasiado grande (>1 MB) para vista previa]".into(), entry.sha));
    }
    let bytes = BASE64.decode(raw_b64).map_err(|e| format!("Error Base64: {}", e))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    Ok((text, entry.sha))
}

pub async fn commit_file_push(
    token: String,
    full_name: String,
    branch: String,
    path: String,
    sha: String,
    new_content: String,
    message: String,
) -> Result<String, String> {
    let (client, api) = build_client(&token)?;
    let clean_path = encode_url_segment(path.trim_start_matches('/'));
    let url = format!("{}/repos/{}/contents/{}", api, full_name, clean_path);
    let encoded = BASE64.encode(new_content.as_bytes());
    let body = json!({
        "message": message,
        "content": encoded,
        "sha": sha,
        "branch": branch
    });
    let resp = client.put(&url).json(&body).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!("Error en Commit/Push: {}", txt));
    }
    let val: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let next_sha = val["content"]["sha"].as_str().unwrap_or_default().to_string();
    Ok(next_sha)
}
