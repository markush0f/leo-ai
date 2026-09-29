//! Tool registration and execution during a chat turn.
//!
//! [`Registry::from_env`] gathers local tools and configured integrations
//! (including MCP Toolbox when `MCP_TOOLBOX_URL` is set).
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
pub use mcp::{attach_configured, attach_mcp, file_servers};
pub use context::Context;
pub use error::ToolError;
pub use catalog::is_mutating;
pub use registry::{Registry, ToolProvider};
pub use tool::{DynTool, Tool};
