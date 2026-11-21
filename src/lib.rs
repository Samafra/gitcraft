pub mod app;
pub mod github;
pub mod github_manage;
pub mod i18n;
// El motor Git local requiere acceso a procesos del sistema: no existe en WASM.
#[cfg(not(target_arch = "wasm32"))]
pub mod local_git;
// E2 utiliza comandos Git controlados y validación previa a toda escritura.
#[cfg(not(target_arch = "wasm32"))]
pub mod local_git_actions;
// E3-A consulta historia avanzada y recuperación de referencias sin escribir.
#[cfg(not(target_arch = "wasm32"))]
pub mod local_git_history;
pub mod mcp_client;
#[cfg(not(target_arch = "wasm32"))]
pub mod mcp_server;
pub mod repo_ops;
pub mod ui_changes;
// La interfaz local también queda limitada a aplicaciones de escritorio.
#[cfg(not(target_arch = "wasm32"))]
pub mod ui_local_git;
// E3-A: panel visual de grafo, comparación, blame y referencias.
#[cfg(not(target_arch = "wasm32"))]
pub mod ui_local_history;
pub mod ui_manage;
pub mod ui_mcp;
pub mod ui_repos;

pub use app::GitCraftApp;
