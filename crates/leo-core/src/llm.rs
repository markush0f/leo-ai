//! Synchronous language-model interface consumed by the voice engine.

/// Replies on the engine thread; implementations may block during provider I/O.
pub trait LlmEngine: Send {
    /// Returns reply text or a displayable failure. Empty text ends the voice turn.
    fn reply(&self, user_text: &str) -> Result<String, String>;
}

/// Unconfigured model fallback that reports an error instead of generating text.
pub struct NullLlm;

impl LlmEngine for NullLlm {
    fn reply(&self, _user_text: &str) -> Result<String, String> {
        Err("llm no configurado (falta XAI_API_KEY)".into())
    }
}
