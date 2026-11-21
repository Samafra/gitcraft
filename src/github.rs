// Importamos los tipos para manejar cabeceras HTTP y convertir JSON a estructuras de Rust (serde).
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};

// #[derive(...)] genera código automático en tiempo de compilación (clonar, imprimir, serializar JSON).
// #[serde(default)] indica que si falta un campo en el JSON de GitHub, se rellene con su valor por defecto (0, "", None).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GitHubUser {
    pub login: String,
    pub name: Option<String>, // Option<T> es el equivalente a un valor nullable: puede ser Some(valor) o None.
    pub avatar_url: String,
    pub public_repos: u32, // u32 = entero sin signo de 32 bits.
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GitHubRepo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub private: bool,
    pub html_url: String,
    pub clone_url: String,
    pub default_branch: String,
    pub stargazers_count: u32,
    pub open_issues_count: u32,
    pub language: Option<String>,
    pub created_at: String, // Fecha de creación devuelta por GitHub (ISO 8601)
    pub updated_at: String, // Fecha de última modificación devuelta por GitHub (ISO 8601)
}

// Función asíncrona que devuelve un Result<Éxito, Error>.
// En Rust no existen excepciones (try/catch); los errores se devuelven explícitamente como valores.
pub async fn fetch_user_and_repos(token: String) -> Result<(GitHubUser, Vec<GitHubRepo>), String> {
    // Eliminamos espacios o comillas accidentales al principio o final del token.
    let clean_token = token.trim().trim_matches('"').trim_matches('\'');

    // Construimos la URL base concatenando partes para evitar que el navegador altere enlaces al copiar.
    let api_base = ["https://", "api.", "github.com"].concat();

    // Configuramos las cabeceras HTTP obligatorias que exige la API REST de GitHub.
    let mut headers = HeaderMap::new();
    let auth_val = HeaderValue::from_str(&format!("Bearer {}", clean_token))
        .map_err(|_| "El token contiene caracteres inválidos.".to_string())?;

    headers.insert(AUTHORIZATION, auth_val);
    headers.insert(USER_AGENT, HeaderValue::from_static("GitCraft-Rust-Client"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    headers.insert(
        "X-GitHub-Api-Version",
        HeaderValue::from_static("2022-11-28"),
    );

    // Creamos el cliente HTTP con esas cabeceras aplicadas a todas las peticiones.
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Error creando cliente HTTP: {}", e))?;

    // 1. Solicitamos los datos del usuario autenticado al endpoint /user.
    let user_url = format!("{}/user", api_base);
    let user_resp = client
        .get(&user_url)
        .send()
        .await
        .map_err(|e| format!("Error de red en /user: {}", e))?;

    let status = user_resp.status();
    let raw_user_text = user_resp
        .text()
        .await
        .map_err(|e| format!("Error leyendo respuesta /user: {}", e))?;

    if !status.is_success() {
        return Err(format!(
            "GitHub rechazó el token ({}): {}",
            status, raw_user_text
        ));
    }

    // Convertimos el texto JSON en nuestra estructura GitHubUser.
    let user: GitHubUser = serde_json::from_str(&raw_user_text)
        .map_err(|e| format!("Error parseando perfil ({}): {}", e, raw_user_text))?;

    if user.login.is_empty() {
        return Err(format!(
            "Respuesta sin login desde {}: {}",
            user_url, raw_user_text
        ));
    }

    // 2. Solicitamos la lista de repositorios (públicos y privados) del usuario autenticado.
    let repos_url = format!("{}/user/repos?per_page=100&sort=updated", api_base);
    let repos_resp = client
        .get(&repos_url)
        .send()
        .await
        .map_err(|e| format!("Error de red en /user/repos: {}", e))?;

    let repos_status = repos_resp.status();
    let raw_repos_text = repos_resp
        .text()
        .await
        .map_err(|e| format!("Error leyendo respuesta /user/repos: {}", e))?;

    if !repos_status.is_success() {
        return Err(format!(
            "Error en /user/repos ({}): {}",
            repos_status, raw_repos_text
        ));
    }

    // Convertimos el array JSON en un vector (lista dinámica Vec<GitHubRepo>).
    let repos: Vec<GitHubRepo> = serde_json::from_str(&raw_repos_text)
        .map_err(|e| format!("Error parseando repos ({}): {}", e, raw_repos_text))?;

    Ok((user, repos))
}
