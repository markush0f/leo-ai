pub trait LlmEngine: Send {
    fn reply(&self, user_text: &str) -> Result<String, String>;
}

pub struct NullLlm;

impl LlmEngine for NullLlm {
    fn reply(&self, _user_text: &str) -> Result<String, String> {
        Err("llm no configurado (falta XAI_API_KEY)".into())
    }
}
