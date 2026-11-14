// Servidor estándar Model Context Protocol (MCP) sobre Stdio (JSON-RPC 2.0).
// Permite que Claude Desktop, OpenAI Codex CLI o cualquier cliente MCP controle GitHub a través de GitCraft.
use crate::github::fetch_user_and_repos;
// use crate::github_manage::create_issue;
use crate::repo_ops::fetch_file_content;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

// Bucle principal del servidor MCP: lee peticiones JSON-RPC línea a línea desde stdin y responde por stdout.
pub async fn run_mcp_server() {
    // Leemos el token desde la variable de entorno que le pasarán Claude Desktop o Codex CLI.
    let token = std::env::var("GITHUB_PERSONAL_ACCESS_TOKEN")
        .or_else(|_| std::env::var("GITHUB_TOKEN"))
        .unwrap_or_default();

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut lines = BufReader::new(stdin).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        let Ok(req) = serde_json::from_str::<Value>(trimmed) else { continue };
        let method = req["method"].as_str().unwrap_or("");
        let id = req.get("id").cloned();

        // Las notificaciones (sin campo 'id') no llevan respuesta en JSON-RPC 2.0.
        let Some(req_id) = id else { continue };

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "gitcraft-mcp-server", "version": "0.3.0" }
                }
            }),
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": { "tools": get_exposed_tools() }
            }),
            "tools/call" => handle_tool_call(req_id, &req["params"], &token).await,
            _ => json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "error": { "code": -32601, "message": format!("Método no soportado: {}", method) }
            }),
        };

        let out = format!("{}\n", serde_json::to_string(&response).unwrap_or_default());
        let _ = stdout.write_all(out.as_bytes()).await;
        let _ = stdout.flush().await;
    }
}
// Define el catálogo de 5 herramientas que Claude y Codex verán disponibles al conectarse a GitCraft.
fn get_exposed_tools() -> Value {
    json!([
        {
            "name": "gitcraft_list_repos",
            "description": "Lista los repositorios públicos y privados del usuario autenticado en GitHub.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "gitcraft_read_file",
            "description": "Lee el contenido y el SHA actual de un archivo dentro de un repositorio de GitHub.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "full_name": { "type": "string", "description": "Usuario/repositorio (ej. samafra/gitcraft)" },
                    "branch": { "type": "string", "description": "Nombre de la rama (ej. main)" },
                    "path": { "type": "string", "description": "Ruta del archivo (ej. README.md)" }
                },
                "required": ["full_name", "branch", "path"]
            }
        },
        {
            "name": "gitcraft_commit_file",
            "description": "Actualiza un archivo en GitHub haciendo Commit & Push directo a la rama indicada.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "full_name": { "type": "string", "description": "Usuario/repositorio" },
                    "branch": { "type": "string", "description": "Rama destino (ej. main)" },
                    "path": { "type": "string", "description": "Ruta del archivo" },
                    "content": { "type": "string", "description": "Nuevo contenido completo del archivo" },
                    "message": { "type": "string", "description": "Mensaje del commit" }
                },
                "required": ["full_name", "branch", "path", "content", "message"]
            }
        },
        {
            "name": "gitcraft_create_issue",
            "description": "Crea un nuevo Issue en un repositorio de GitHub.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "full_name": { "type": "string", "description": "Usuario/repositorio" },
                    "title": { "type": "string", "description": "Título del Issue" },
                    "body": { "type": "string", "description": "Descripción detallada" }
                },
                "required": ["full_name", "title"]
            }
        },
        {
            "name": "gitcraft_merge_pr",
            "description": "Fusiona (Merge) un Pull Request abierto en un repositorio de GitHub.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "full_name": { "type": "string", "description": "Usuario/repositorio" },
                    "pr_number": { "type": "integer", "description": "Número del Pull Request" }
                },
                "required": ["full_name", "pr_number"]
            }
        }
    ])
}

// Ejecuta la herramienta solicitada por el agente de IA y devuelve el resultado en formato MCP estándar.
async fn handle_tool_call(id: Value, params: &Value, token: &str) -> Value {
    if token.trim().is_empty() {
        return json!({
            "jsonrpc": "2.0", "id": id,
            "result": { "isError": true, "content": [{ "type": "text", "text": "Falta la variable GITHUB_PERSONAL_ACCESS_TOKEN" }] }
        });
    }

    let name = params["name"].as_str().unwrap_or("");
    let args = &params["arguments"];

    let exec_result: Result<String, String> = match name {
        "gitcraft_list_repos" => {
            fetch_user_and_repos(token.to_string()).await.map(|(u, repos)| {
                let list: Vec<String> = repos.iter().map(|r| format!("{} ({})", r.full_name, r.default_branch)).collect();
                format!("Usuario: @{}\nRepositorios:\n{}", u.login, list.join("\n"))
            })
        }
        "gitcraft_read_file" => {
            let repo = args["full_name"].as_str().unwrap_or("").to_string();
            let branch = args["branch"].as_str().unwrap_or("main").to_string();
            let path = args["path"].as_str().unwrap_or("").to_string();
            fetch_file_content(token.to_string(), repo, branch, path).await.map(|(txt, sha)| format!("SHA: {}\n---\n{}", sha, txt))
        }
        "gitcraft_commit_file" => {
            let repo = args["full_name"].as_str().unwrap_or("").to_string();
            let branch = args["branch"].as_str().unwrap_or("main").to_string();
            let path = args["path"].as_str().unwrap_or("").to_string();
            let content = args["content"].as_str().unwrap_or("").to_string();
            let msg = args["message"].as_str().unwrap_or("Update via GitCraft MCP").to_string();
            // Obtenemos primero el SHA actual del archivo antes de hacer Commit & Push.
            match fetch_file_content(token.to_string(), repo.clone(), branch.clone(), path.clone()).await {
                Ok((_, sha)) => crate::repo_ops::commit_file_push(token.to_string(), repo, branch, path, sha, content, msg)
                    .await
                    .map(|new_sha| format!("Commit & Push realizado con éxito. Nuevo SHA: {}", new_sha)),
                Err(e) => Err(e),
            }
        }
        "gitcraft_create_issue" => {
            let repo = args["full_name"].as_str().unwrap_or("").to_string();
            let title = args["title"].as_str().unwrap_or("").to_string();
            let body = args["body"].as_str().unwrap_or("").to_string();
            crate::github_manage::create_issue(token.to_string(), repo, title, body).await.map(|_| "Issue creado correctamente en GitHub.".into())
        }
        "gitcraft_merge_pr" => {
            let repo = args["full_name"].as_str().unwrap_or("").to_string();
            let num = args["pr_number"].as_u64().unwrap_or(0);
            crate::github_manage::merge_pr(token.to_string(), repo, num).await.map(|_| format!("Pull Request #{} fusionado correctamente.", num))
        }
        _ => Err(format!("Herramienta desconocida: {}", name)),
    };

    match exec_result {
        Ok(text) => json!({ "jsonrpc": "2.0", "id": id, "result": { "content": [{ "type": "text", "text": text }] } }),
        Err(err) => json!({ "jsonrpc": "2.0", "id": id, "result": { "isError": true, "content": [{ "type": "text", "text": err }] } }),
    }
}
