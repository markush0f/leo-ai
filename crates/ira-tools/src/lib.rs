//! Tool registration and execution during a chat turn.
//!
//! [`Registry::from_env`] gathers local tools and configured integrations
//! (including MCP Toolbox when `MCP_TOOLBOX_URL` is set, and Veritas Kanban
//! when `VERITAS_MCP_URL` is set).
//! [`chat`] feeds their results back to the model until a final response or the
//! round limit is reached. Each integration lives in a separate crate.

#![warn(missing_docs)]

mod args;
mod catalog;
mod chat;
mod context;
mod error;
mod mcp;
mod registry;
mod tool;

pub use chat::{StreamEvent, StreamSink, chat, chat_stream, run};
pub use mcp::attach_mcp;
pub use context::Context;
pub use error::ToolError;
pub use registry::Registry;
pub use tool::{DynTool, Tool};
