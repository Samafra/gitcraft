pub mod app;
pub mod github;
pub mod github_manage;
pub mod i18n;
// El motor Git local requiere acceso a procesos del sistema: no existe en WASM.
#[cfg(not(target_arch = "wasm32"))]
pub mod local_git;
pub mod mcp_client;
#[cfg(not(target_arch = "wasm32"))]
pub mod mcp_server;
pub mod repo_ops;
pub mod ui_changes;
// La interfaz local también queda limitada a aplicaciones de escritorio.
#[cfg(not(target_arch = "wasm32"))]
pub mod ui_local_git;
pub mod ui_manage;
pub mod ui_mcp;
pub mod ui_repos;

pub use app::GitCraftApp;
