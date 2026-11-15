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

// Representa un archivo o carpeta dentro de un repositorio junto con su último commit.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RepoEntry {
    pub name: String,
    pub path: String,
    pub sha: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub content: Option<String>,
    pub last_commit_msg: String,  // Mensaje del último commit que tocó este archivo/carpeta
    pub last_commit_hash: String, // Hash corto de 7 caracteres (ej. "a1b2c3d")
    pub last_commit_date: String, // Fecha formateada del último commit (ej. "2026-11-15 00:40")
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

// Función auxiliar que consulta el último commit (mensaje, hash corto y fecha) de un archivo o carpeta.
async fn enrich_with_last_commit(
    client: reqwest::Client,
    api_url: String,
    repo_name: String,
    branch: String,
    mut item: RepoEntry,
) -> RepoEntry {
    let ep = encode_url_segment(&item.path);
    let commit_url = format!("{}/repos/{}/commits?sha={}&path={}&per_page=1", api_url, repo_name, branch, ep);
    if let Ok(r) = client.get(&commit_url).send().await {
        if let Ok(vals) = r.json::<Vec<serde_json::Value>>().await {
            if let Some(first) = vals.first() {
                let full_sha = first["sha"].as_str().unwrap_or("");
                item.last_commit_hash = full_sha.chars().take(7).collect();
                let raw_msg = first["commit"]["message"].as_str().unwrap_or("-");
                item.last_commit_msg = raw_msg.lines().next().unwrap_or("-").to_string();
                let raw_date = first["commit"]["committer"]["date"].as_str().unwrap_or("");
                item.last_commit_date = if raw_date.len() >= 16 {
                    raw_date[..16].replace('T', " ")
                } else {
                    "-".to_string()
                };
            }
        }
    }
    item
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
    if resp.status().as_u16() == 404 && path.is_empty() {
        return Ok(Vec::new());
    }
    if !resp.status().is_success() {
        return Err(format!("Error listando archivos ({}) en '{}'", resp.status(), path));
    }

    let mut entries = resp.json::<Vec<RepoEntry>>().await.map_err(|e| e.to_string())?;
    // Ordenamos: primero carpetas ("dir"), luego archivos ("file") alfabéticamente.
    entries.sort_by(|a, b| b.entry_type.cmp(&a.entry_type).then(a.name.cmp(&b.name)));

    // En Escritorio ejecutamos las peticiones en paralelo con tokio::spawn.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut handles = Vec::new();
        for entry in entries {
            handles.push(tokio::spawn(enrich_with_last_commit(
                client.clone(),
                api.clone(),
                full_name.clone(),
                clean_branch.clone(),
                entry,
            )));
        }
        let mut enriched = Vec::with_capacity(handles.len());
        for h in handles {
            if let Ok(item) = h.await {
                enriched.push(item);
            }
        }
        Ok(enriched)
    }

    // En WebAssembly (wasm32) ejecutamos las peticiones asíncronas directamente en el hilo del navegador.
    #[cfg(target_arch = "wasm32")]
    {
        let mut enriched = Vec::with_capacity(entries.len());
        for entry in entries {
            let item = enrich_with_last_commit(
                client.clone(),
                api.clone(),
                full_name.clone(),
                clean_branch.clone(),
                entry,
            )
            .await;
            enriched.push(item);
        }
        Ok(enriched)
    }
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
