//! Cliente MCP sin lógica por servidor.
//!
//! [`McpManager`] solo ve [`McpServerConfig`]. `add` registra un comando o URL
//! conocidos. `install` resuelve una receta local o el registry oficial y
//! produce el mismo modelo.

mod config_file;
mod error;
mod http;
mod import;
mod install;
mod manager;
mod recipe;
mod registry;
mod rpc;
mod stdio;
mod vars;

pub use config_file::{Scope, enabled_servers, global_path, load_all, project_file, remove, upsert};
pub use error::Error;
pub use import::{from_add, load_file, parse_claude};
pub use install::resolve;
pub use ira_store::{McpServerConfig, McpTransport};
pub use manager::{McpManager, shared};
pub use registry::McpRegistryClient;
pub use rpc::RemoteTool;
pub use vars::{resolve_config, slug};
