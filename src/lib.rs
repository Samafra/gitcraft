pub mod app;
pub mod github;
pub mod github_manage;
pub mod i18n;
pub mod mcp_client;
#[cfg(not(target_arch = "wasm32"))]
pub mod mcp_server;
pub mod repo_ops;
pub mod ui_changes;
pub mod ui_manage;
pub mod ui_mcp;
pub mod ui_repos;

pub use app::GitCraftApp;
