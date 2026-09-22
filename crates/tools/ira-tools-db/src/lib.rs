//! Database tools backed by a **local** [MCP Toolbox](https://github.com/googleapis/mcp-toolbox)
//! checkout (`third_party/mcp-toolbox`).
//!
//! `ira-tools` registers these operations when `MCP_TOOLBOX_URL` or
//! `TOOLBOX_URL` points at the Compose service (publishes `127.0.0.1:5000`).
//! The client speaks MCP JSON-RPC over HTTP (`/mcp`, optional toolset path).
//! Toolbox owns pooling and the actual SQL; this crate only discovers and
//! invokes those tools.
//!
//! Convenience operations (`db_execute_sql`, `db_list_tables`, …) map to the
//! prebuilt generic names. Custom toolsets are reachable through
//! `db_list_tools` and `db_invoke`.

/// MCP Toolbox HTTP transport.
pub mod client;
/// Server-wide database overview (prebuilt `database_overview`).
pub mod database_overview;
mod error;
/// Executes SQL (prebuilt `execute_sql`).
pub mod execute_sql;
/// Query plan (prebuilt `get_query_plan`).
pub mod get_query_plan;
/// Invokes any Toolbox tool by name.
pub mod invoke;
/// Lists schemas (prebuilt `list_schemas`).
pub mod list_schemas;
/// Lists tables (prebuilt `list_tables`).
pub mod list_tables;
/// Lists tools advertised by the Toolbox server.
pub mod list_tools;
/// Lists views (prebuilt `list_views`).
pub mod list_views;

pub use client::{Client, RemoteTool};
pub use error::Error;
