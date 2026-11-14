use crate::app::GitCraftApp;
use crate::mcp_client::McpTransport;
use eframe::egui;

// Dibuja la interfaz del Cliente MCP con desplegable de conexiones predefinidas + caja manual.
pub fn render_mcp_ui(app: &mut GitCraftApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let lang = app.language;
    ui.heading(app.i18n.t(lang, "mcp_panel_title"));
    ui.separator();

    // Selector de transporte: Stdio (CLI local) o HTTP/JSON-RPC (Web/Remoto).
    ui.horizontal_wrapped(|ui| {
        ui.label(app.i18n.t(lang, "mcp_transport_label"));
        ui.selectable_value(&mut app.mcp_transport, McpTransport::Stdio, "Stdio (CLI)");
        ui.selectable_value(&mut app.mcp_transport, McpTransport::Http, "HTTP / JSON-RPC");
    });

    // Resolvemos la carpeta HOME del usuario (/Users/francisco) para que el binario funcione desde cualquier directorio.
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let release_bin = format!("{}/Projects/gitcraft/target/release/gitcraft --mcp-server", home);
    let dev_cargo = format!("cargo run --manifest-path {}/Projects/gitcraft/Cargo.toml -- --mcp-server", home);

    let presets: Vec<(&str, McpTransport, String)> = vec![
        ("🦀 GitCraft (Binario Release)", McpTransport::Stdio, release_bin),
        ("🦀 GitCraft (Modo Cargo Dev)", McpTransport::Stdio, dev_cargo),
        ("🐙 GitHub MCP Oficial (npx)", McpTransport::Stdio, "npx -y @modelcontextprotocol/server-github".into()),
        ("📂 Archivos Locales /Users (npx)", McpTransport::Stdio, "npx -y @modelcontextprotocol/server-filesystem /Users".into()),
        ("🧠 Memoria Persistente (npx)", McpTransport::Stdio, "npx -y @modelcontextprotocol/server-memory".into()),
        ("🌐 Servidor HTTP Local (8080)", McpTransport::Http, "http://localhost:8080/mcp".into()),
    ];

    ui.horizontal(|ui| {
        ui.label(app.i18n.t(lang, "mcp_presets_label"));
        egui::ComboBox::from_id_salt("mcp_presets_combo")
            .selected_text("Seleccionar servidor...")
            .show_ui(ui, |ui| {
                for (label, transport, cmd) in &presets {
                    if ui.selectable_label(app.mcp_endpoint == *cmd, *label).clicked() {
                        app.mcp_transport = *transport;
                        app.mcp_endpoint = cmd.clone();
                    }
                }
            });
    });

    // Caja de texto manual (se rellena sola al elegir del desplegable o permite escribir libremente).
    ui.add(
        egui::TextEdit::singleline(&mut app.mcp_endpoint)
            .desired_width(f32::INFINITY)
            .hint_text(app.i18n.t(lang, "mcp_cmd_hint")),
    );

    if ui.button(app.i18n.t(lang, "mcp_connect_btn")).clicked() && !app.mcp_endpoint.trim().is_empty() {
        app.trigger_mcp_discover(ctx.clone());
    }

    ui.separator();
    // Lista de herramientas expuestas por el servidor MCP conectado.
    if !app.mcp_tools.is_empty() {
        ui.label(egui::RichText::new(app.i18n.t(lang, "mcp_tools_header")).strong());
        egui::ComboBox::from_id_salt("mcp_tool_selector")
            .selected_text(&app.mcp_selected_tool)
            .show_ui(ui, |ui| {
                for tool in &app.mcp_tools.clone() {
                    if ui.selectable_value(&mut app.mcp_selected_tool, tool.name.clone(), &tool.name).clicked() {
                        if let Some(repo) = &app.selected_repo {
                            let parts: Vec<&str> = repo.full_name.split('/').collect();
                            if parts.len() == 2 {
                                app.mcp_tool_args = format!("{{\"owner\": \"{}\", \"repo\": \"{}\", \"full_name\": \"{}\"}}", parts[0], parts[1], repo.full_name);
                            }
                        }
                    }
                }
            });

        if let Some(t) = app.mcp_tools.iter().find(|t| t.name == app.mcp_selected_tool) {
            if let Some(desc) = &t.description {
                ui.label(egui::RichText::new(desc).weak().small());
            }
        }

        ui.add_space(6.0);
        ui.label(app.i18n.t(lang, "mcp_args_label"));
        ui.add(
            egui::TextEdit::multiline(&mut app.mcp_tool_args)
                .code_editor()
                .desired_width(f32::INFINITY)
                .desired_rows(4),
        );

        if ui.button(app.i18n.t(lang, "mcp_run_tool_btn")).clicked() && !app.mcp_selected_tool.is_empty() {
            app.trigger_mcp_call(ctx.clone());
        }
    }

    if !app.mcp_last_output.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new(app.i18n.t(lang, "mcp_output_header")).strong());
        egui::ScrollArea::vertical().id_salt("mcp_out_scroll").max_height(240.0).show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut app.mcp_last_output)
                    .code_editor()
                    .desired_width(f32::INFINITY),
            );
        });
    }
}
