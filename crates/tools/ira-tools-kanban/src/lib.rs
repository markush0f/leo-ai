//! Veritas Kanban tools backed by the local MCP gateway in
//! `services/veritas-kanban`.
//!
//! `ira-tools` registers these operations when `VERITAS_MCP_URL` points at the
//! Compose service (publishes `127.0.0.1:3100`). The client speaks MCP JSON-RPC
//! over streamable HTTP (`/mcp`). The gateway owns the stdio MCP process.

/// MCP streamable HTTP transport.
pub mod client;
mod error;
/// Invokes any Veritas Kanban MCP tool by its remote name.
pub mod invoke;
/// Lists tools advertised by the Kanban MCP server.
pub mod list_tools;
/// Board operations mapped onto remote MCP tools.
pub mod tasks;

pub use client::{Client, RemoteTool};
pub use error::Error;
