// Cliente estándar Model Context Protocol (MCP) basado en JSON-RPC 2.0.
// Implementa el handshake secuencial oficial: initialize -> respuesta -> notifications/initialized -> tools/list.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpTransport {
    Stdio, // Ejecuta un binario o script local (codex mcp-server, npx, uvx, python).
    Http,  // Se conecta a un servidor MCP vía HTTP POST JSON-RPC (compatible con Web/WASM).
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

pub async fn mcp_discover_tools(
    transport: McpTransport,
    endpoint: String,
    github_token: String,
) -> Result<Vec<McpTool>, String> {
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "GitCraft", "version": "0.3.0" }
        }
    });
    let notify_req = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let list_req = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} });

    let raw_resp = send_mcp_session(transport, &endpoint, &github_token, init_req, notify_req, list_req).await?;

    let tools_val = raw_resp
        .iter()
        .find(|v| v["id"] == 2)
        .and_then(|v| v["result"]["tools"].as_array())
        .ok_or_else(|| format!("Respuesta inesperada del servidor MCP: {:?}", raw_resp))?;

    serde_json::from_value(Value::Array(tools_val.clone()))
        .map_err(|e| format!("Error parseando herramientas MCP: {}", e))
}
pub async fn mcp_call_tool(
    transport: McpTransport,
    endpoint: String,
    github_token: String,
    tool_name: String,
    args_json: String,
) -> Result<String, String> {
    let args: Value = serde_json::from_str(&args_json)
        .map_err(|e| format!("JSON de argumentos inválido: {}", e))?;

    let init_req = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": { "name": "GitCraft", "version": "0.3.0" } }
    });
    let notify_req = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let call_req = json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": { "name": tool_name, "arguments": args }
    });

    let raw_resp = send_mcp_session(transport, &endpoint, &github_token, init_req, notify_req, call_req).await?;
    let result = raw_resp.iter().find(|v| v["id"] == 3).cloned().unwrap_or_else(|| json!(raw_resp));
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

async fn send_mcp_session(
    transport: McpTransport,
    endpoint: &str,
    github_token: &str,
    init_req: Value,
    notify_req: Value,
    action_req: Value,
) -> Result<Vec<Value>, String> {
    match transport {
        McpTransport::Http => {
            let client = reqwest::Client::new();
            let mut responses = Vec::new();
            for msg in [init_req, notify_req, action_req] {
                let is_notif = msg.get("id").is_none();
                let resp = client.post(endpoint.trim()).header("Content-Type", "application/json")
                    .header("Accept", "application/json, text/event-stream").json(&msg).send().await
                    .map_err(|e| format!("Error HTTP MCP: {}", e))?;
                if !is_notif {
                    if let Ok(val) = serde_json::from_str::<Value>(&resp.text().await.unwrap_or_default()) {
                        responses.push(val);
                    }
                }
            }
            Ok(responses)
        }
        McpTransport::Stdio => {
            #[cfg(target_arch = "wasm32")]
            {
                let _ = (endpoint, github_token, init_req, notify_req, action_req);
                Err("Stdio solo funciona en escritorio; usa HTTP en Web.".into())
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
                use tokio::process::Command;

                let parts: Vec<&str> = endpoint.split_whitespace().collect();
                let Some((prog, args)) = parts.split_first() else {
                    return Err("Indica el comando del servidor MCP.".into());
                };

                // Ampliamos el PATH para encontrar node, npx, uvx o codex en macOS.
                let sys_path = std::env::var("PATH").unwrap_or_default();
                let full_path = format!("/opt/homebrew/bin:/usr/local/bin:{}", sys_path);

                // Eliminamos explícitamente posibles tokens heredados del proceso padre.
                // Un servidor MCP no autorizado nunca recibe automáticamente credenciales GitHub.
                let mut command = Command::new(prog);
                command.args(args)
                    .env("PATH", full_path)
                    .env_remove("GITHUB_PERSONAL_ACCESS_TOKEN")
                    .env_remove("GITHUB_TOKEN")
                    .env_remove("GH_TOKEN");

                // Sólo cuando el usuario ha autorizado este servidor se añade el PAT.
                // Nunca se escribe el token en el comando, argumentos ni mensajes de error.
                if !github_token.trim().is_empty() {
                    command.env("GITHUB_PERSONAL_ACCESS_TOKEN", github_token.trim())
                           .env("GITHUB_TOKEN", github_token.trim());
                }

                let mut child = command
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| format!("No se pudo ejecutar '{}': {}", prog, e))?;

                let mut stdin = child.stdin.take().ok_or("Sin stdin")?;
                let stdout = child.stdout.take().ok_or("Sin stdout")?;
                let mut stderr = child.stderr.take().ok_or("Sin stderr")?;
                let mut reader = BufReader::new(stdout).lines();

                // Paso 1: Enviamos 'initialize' y esperamos su respuesta antes de continuar.
                stdin.write_all(format!("{}\n", init_req).as_bytes()).await.map_err(|e| e.to_string())?;
                stdin.flush().await.map_err(|e| e.to_string())?;

                let mut responses = Vec::new();
                while let Ok(Ok(Some(line))) = tokio::time::timeout(std::time::Duration::from_secs(30), reader.next_line()).await {
                    if let Ok(val) = serde_json::from_str::<Value>(&line) {
                        if val.get("id") == Some(&json!(1)) {
                            responses.push(val);
                            break;
                        }
                    }
                }

                // Paso 2: Enviamos 'notifications/initialized' y la petición real; si el proceso cerró, leemos stderr.
                if let Err(e) = stdin.write_all(format!("{}\n{}\n", notify_req, action_req).as_bytes()).await {
                    let mut err_out = String::new();
                    let _ = stderr.read_to_string(&mut err_out).await;
                    return Err(format!("El proceso MCP terminó ({}): {}", e, err_out.trim()));
                }
                stdin.flush().await.map_err(|e| e.to_string())?;

                let target_id = action_req["id"].clone();
                while let Ok(Ok(Some(line))) = tokio::time::timeout(std::time::Duration::from_secs(45), reader.next_line()).await {
                    if let Ok(val) = serde_json::from_str::<Value>(&line) {
                        if val.get("id") == Some(&target_id) {
                            responses.push(val);
                            break;
                        }
                    }
                }

                let _ = child.kill().await;
                if responses.len() < 2 {
                    let mut err_out = String::new();
                    let _ = stderr.read_to_string(&mut err_out).await;
                    return Err(format!("El servidor MCP cerró sin responder. Detalle: {}", err_out.trim()));
                }
                Ok(responses)
            }
        }
    }
}
