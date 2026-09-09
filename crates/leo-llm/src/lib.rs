mod claude;
mod client;
mod error;
mod ollama;
mod openai_compat;
mod types;

pub use client::{extract_error_message, Client};
pub use error::LlmError;
pub use ollama::parse_tags as parse_ollama_tags;
pub use types::{ChatMessage, ChatRequest, ChatResponse, ProviderId, Role};

/// Construcción y parseo de payloads HTTP. La usa el cliente y los tests.
pub mod protocol {
    pub use crate::claude::{build_body as claude_body, parse_response as parse_claude};
    pub use crate::ollama::{build_body as ollama_body, parse_response as parse_ollama};
    pub use crate::openai_compat::{build_body as openai_body, parse_response as parse_openai};
}
