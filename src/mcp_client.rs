//! Cliente MCP de GitCraft.
//!
//! MCP permite pedir a programas o servicios externos que ejecuten herramientas.
//! Se distingue entre Stdio (proceso local, sólo escritorio) y HTTP (servicio remoto).
//! El token GitHub nunca debe compartirse con procesos por defecto.
// Cliente estándar Model Context Protocol (MCP) basado en JSON-RPC 2.0.
// Implementa el handshake secuencial oficial: initialize -> respuesta -> notifications/initialized -> tools/list.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Medio de comunicación con un servidor MCP.
pub enum McpTransport {
    Stdio, // Ejecuta un binario o script local (codex mcp-server, npx, uvx, python).
    Http,  // Se conecta a un servidor MCP vía HTTP POST JSON-RPC (compatible con Web/WASM).
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
/// Nombre, descripción y esquema JSON de una herramienta disponible en un servidor MCP.
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Negocia la conexión y solicita al servidor la lista de herramientas que ofrece.
/// En caso de fallo devuelve `Err` con información comprensible.
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

    let raw_resp = send_mcp_session(
        transport,
        &endpoint,
        &github_token,
        init_req,
        notify_req,
        list_req,
    )
    .await?;

    let tools_val = raw_resp
        .iter()
        .find(|v| v["id"] == 2)
        .and_then(|v| v["result"]["tools"].as_array())
        .ok_or_else(|| format!("Respuesta inesperada del servidor MCP: {:?}", raw_resp))?;

    serde_json::from_value(Value::Array(tools_val.clone()))
        .map_err(|e| format!("Error parseando herramientas MCP: {}", e))
}
/// Ejecuta por nombre una herramienta MCP y devuelve su respuesta en JSON legible.
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

    let raw_resp = send_mcp_session(
        transport,
        &endpoint,
        &github_token,
        init_req,
        notify_req,
        call_req,
    )
    .await?;
    let result = raw_resp
        .iter()
        .find(|v| v["id"] == 3)
        .cloned()
        .unwrap_or_else(|| json!(raw_resp));
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

/// Intercambia mensajes con un servidor MCP. `async` permite esperar sin bloquear toda la UI.
/// El tratamiento Stdio y HTTP difiere, especialmente en WebAssembly.
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
                let resp = client
                    .post(endpoint.trim())
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json, text/event-stream")
                    .json(&msg)
                    .send()
                    .await
                    .map_err(|e| format!("Error HTTP MCP: {}", e))?;
                if !is_notif {
                    if let Ok(val) =
                        serde_json::from_str::<Value>(&resp.text().await.unwrap_or_default())
                    {
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

                // Separamos la política de credenciales en una función verificable.
                // Así usamos exactamente las mismas reglas en el programa y en sus tests.
                let mut command = Command::new(prog);
                command.args(args).env("PATH", full_path);
                configure_mcp_github_environment(&mut command, github_token);

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
                stdin
                    .write_all(format!("{}\n", init_req).as_bytes())
                    .await
                    .map_err(|e| e.to_string())?;
                stdin.flush().await.map_err(|e| e.to_string())?;

                let mut responses = Vec::new();
                while let Ok(Ok(Some(line))) =
                    tokio::time::timeout(std::time::Duration::from_secs(30), reader.next_line())
                        .await
                {
                    if let Ok(val) = serde_json::from_str::<Value>(&line) {
                        if val.get("id") == Some(&json!(1)) {
                            responses.push(val);
                            break;
                        }
                    }
                }

                // Paso 2: Enviamos 'notifications/initialized' y la petición real; si el proceso cerró, leemos stderr.
                if let Err(e) = stdin
                    .write_all(format!("{}\n{}\n", notify_req, action_req).as_bytes())
                    .await
                {
                    let mut err_out = String::new();
                    let _ = stderr.read_to_string(&mut err_out).await;
                    return Err(format!(
                        "El proceso MCP terminó ({}): {}",
                        e,
                        err_out.trim()
                    ));
                }
                stdin.flush().await.map_err(|e| e.to_string())?;

                let target_id = action_req["id"].clone();
                while let Ok(Ok(Some(line))) =
                    tokio::time::timeout(std::time::Duration::from_secs(45), reader.next_line())
                        .await
                {
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
                    return Err(format!(
                        "El servidor MCP cerró sin responder. Detalle: {}",
                        err_out.trim()
                    ));
                }
                Ok(responses)
            }
        }
    }
}

/// Prepara las variables de entorno de un proceso MCP en sistemas de escritorio.
///
/// En Rust, un `Command` representa un programa que se ejecutará más adelante.
/// `env_remove` impide que el proceso reciba una variable incluso si GitCraft
/// la heredó de Terminal o de otra aplicación. Es importante realizarlo **antes**
/// de iniciar el proceso, para no exponer por accidente un token GitHub.
///
/// Parámetros:
/// - `command`: programa hijo que todavía no ha sido iniciado.
/// - `github_token`: token expresamente autorizado por el usuario para este
///   servidor concreto. Una cadena vacía significa «sin autorización».
///
/// Esta función no ejecuta programas, no escribe ficheros y no muestra secretos.
#[cfg(not(target_arch = "wasm32"))]
fn configure_mcp_github_environment(command: &mut tokio::process::Command, github_token: &str) {
    // Primero revocamos cualquier credencial de GitHub que pudiera heredarse.
    // También retiramos GH_TOKEN, que utilizan otras herramientas de GitHub.
    command
        .env_remove("GITHUB_PERSONAL_ACCESS_TOKEN")
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN");

    // Sólo el consentimiento explícito de la interfaz puede proporcionar un PAT.
    // Las variables tienen el mismo nombre que utilizaba el cliente anteriormente.
    if !github_token.trim().is_empty() {
        command
            .env("GITHUB_PERSONAL_ACCESS_TOKEN", github_token.trim())
            .env("GITHUB_TOKEN", github_token.trim());
    }
}

// Estas pruebas ejecutan únicamente el programa local «env», que muestra las
// variables recibidas por el proceso hijo. No se invoca GitHub ni ningún servidor MCP.
// La prueba existe para sistemas Unix (macOS/Linux); en Windows deberá cubrirse
// posteriormente con un ejecutable de prueba equivalente.
#[cfg(all(test, unix))]
mod mcp_environment_tests {
    use super::configure_mcp_github_environment;
    use std::collections::BTreeMap;
    use tokio::process::Command;

    /// Arranca un proceso de prueba con valores ficticios en sus variables.
    ///
    /// Primero simulamos credenciales heredadas y después aplicamos la misma
    /// política de seguridad que utiliza GitCraft. La salida se examina en
    /// memoria: jamás se imprime una variable ni un token en el informe.
    async fn probe_environment(token_autorizado: &str) -> BTreeMap<String, String> {
        let mut command = Command::new("env");
        command
            .env("GITHUB_PERSONAL_ACCESS_TOKEN", "E0_DUMMY_HEREDADO_A")
            .env("GITHUB_TOKEN", "E0_DUMMY_HEREDADO_B")
            .env("GH_TOKEN", "E0_DUMMY_HEREDADO_C");

        // Al aplicar la política, todas las credenciales ficticias se borran.
        // Si existe permiso, sólo se agrega el token ficticio autorizado.
        configure_mcp_github_environment(&mut command, token_autorizado);

        let output = command.output().await.expect("debe existir el comando env");
        assert!(
            output.status.success(),
            "el proceso de prueba debe finalizar bien"
        );

        let text = String::from_utf8(output.stdout).expect("salida UTF-8 del proceso de prueba");
        text.lines()
            .filter_map(|line| line.split_once('='))
            .filter(|(key, _)| {
                matches!(
                    *key,
                    "GITHUB_PERSONAL_ACCESS_TOKEN" | "GITHUB_TOKEN" | "GH_TOKEN"
                )
            })
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    /// Sin autorización, ni siquiera los tokens heredados ficticios llegan al hijo.
    #[tokio::test]
    async fn no_autorizado_no_hereda_credenciales_github() {
        let variables = probe_environment("").await;
        assert!(
            variables.is_empty(),
            "un servidor no autorizado no debe recibir variables GitHub"
        );
    }

    /// Con autorización, el proceso recibe sólo el token ficticio indicado.
    /// GH_TOKEN continúa eliminado porque no se usa para esta integración.
    #[tokio::test]
    async fn autorizado_recibe_solo_token_ficticio() {
        let variables = probe_environment("E0_DUMMY_AUTORIZADO").await;
        assert_eq!(
            variables
                .get("GITHUB_PERSONAL_ACCESS_TOKEN")
                .map(String::as_str),
            Some("E0_DUMMY_AUTORIZADO")
        );
        assert_eq!(
            variables.get("GITHUB_TOKEN").map(String::as_str),
            Some("E0_DUMMY_AUTORIZADO")
        );
        assert!(!variables.contains_key("GH_TOKEN"));
    }
}
