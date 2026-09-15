//! Shared chat client for Grok, GPT, Ollama, and Claude.
//!
//! [`ChatRequest`] and [`ChatResponse`] isolate callers from provider-specific
//! HTTP formats. Tool calls are transported as data; `leo-tools` owns their
//! execution and follow-up rounds.

mod claude;
mod client;
mod dotenv;
mod error;
mod ollama;
mod openai_compat;
mod types;

pub use client::{Client, extract_error_message};
pub use dotenv::load_dotenv;
pub use error::LlmError;
pub use ollama::parse_tags as parse_ollama_tags;
pub use types::{ChatMessage, ChatRequest, ChatResponse, ProviderId, Role, ToolCall, ToolSpec};

/// HTTP payload builders and parsers shared by the client and protocol tests.
pub mod protocol {
    pub use crate::claude::{build_body as claude_body, parse_response as parse_claude};
    pub use crate::ollama::{build_body as ollama_body, parse_response as parse_ollama};
    pub use crate::openai_compat::{build_body as openai_body, parse_response as parse_openai};
}
