# 🦀 GitCraft

**GitCraft** es un gestor completo de repositorios de GitHub y cliente **MCP (Model Context Protocol)** escrito en **Rust**, siguiendo la filosofía de las *Crafting Apps* (nativo, sin Electron, ultra ligero y multiplataforma).

## ✨ Características principales
* **Multiplataforma real:** Windows, macOS Universal (`x86_64` + `aarch64`), Linux y WebAssembly (WASM) desde un único código fuente en Rust (`egui` + `eframe` con backend `glow`/OpenGL).
* **Gestión total de GitHub:** Explorador de repositorios, ramas, editor de archivos con Commit & Push directo, Pull Requests (crear y fusionar), Issues, GitHub Actions (CI/CD) y Releases.
* **Cliente MCP estándar (JSON-RPC 2.0):** Compatible con servidores MCP locales (`Stdio`) y remotos (`HTTP/SSE`) como Claude, OpenAI Codex CLI y `@modelcontextprotocol/server-github`.
* **Sistema Multi-idioma JSON:** Traducciones externas en `locales/` (`es.json`, `en.json`) fáciles de editar y ampliar.

## 🚀 Compilación y ejecución local
```bash
# Ejecutar versión nativa de escritorio
cargo run --release

# Ejecutar versión Web en el navegador
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve --open
```

## 📄 Licencia
Distribuido bajo licencia **MIT**.
