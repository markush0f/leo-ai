//! Shared chat client for Grok, GPT, Ollama, and Claude.
//!
//! [`ChatRequest`] and [`ChatResponse`] isolate callers from provider-specific
//! HTTP formats. Tool calls are transported as data; `ira-tools` owns their
//! execution and follow-up rounds.

mod error;
pub mod protocols;
pub mod provider;
pub mod providers;
mod stream;
pub mod types;

pub use error::LlmError;
pub use provider::Provider;
pub use providers::codex::{Codex, OAuthCredentials, TokenStore};
pub use providers::ollama::parse_tags as parse_ollama_tags;
pub use providers::{Client, extract_error_message};
pub use types::{ChatMessage, ChatRequest, ChatResponse, ProviderId, Role, ToolCall, ToolSpec};

/// Callback invoked for each text fragment received from a provider stream.
pub type TextSink = std::sync::Arc<dyn Fn(String) + Send + Sync + 'static>;

/// Loads the first `.env` found while walking up from the working directory.
/// Checks at most 16 directories, preserves exported variables, and ignores load errors.
pub fn load_dotenv() {
    if let Ok(dir) = std::env::current_dir() {
        if let Some(path) = find_dotenv_from(&dir) {
            let _ = dotenvy::from_path(path);
        }
    }
}

fn find_dotenv_from(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut dir = start.to_path_buf();
    for _ in 0..16 {
        let path = dir.join(".env");
        if path.is_file() {
            return Some(path);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// HTTP payload builders and parsers shared by the client and protocol tests.
pub mod protocol {
    pub use crate::protocols::openai_compatible::{
        build_body as openai_body, parse_response as parse_openai,
    };
    pub use crate::providers::claude::{build_body as claude_body, parse_response as parse_claude};
    pub use crate::providers::ollama::{build_body as ollama_body, parse_response as parse_ollama};
}

#[cfg(test)]
mod tests {
    use super::find_dotenv_from;

    #[test]
    fn finds_env_in_parent() {
        let root = std::env::temp_dir().join(format!("ira-dotenv-{}", std::process::id()));
        let nested = root.join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        let env_path = root.join(".env");
        std::fs::write(&env_path, "FOO=bar\n").unwrap();
        let found = find_dotenv_from(&nested).expect("parent .env");
        assert_eq!(found, env_path);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_env_is_none() {
        let root = std::env::temp_dir().join(format!("ira-dotenv-none-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(find_dotenv_from(&root).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
