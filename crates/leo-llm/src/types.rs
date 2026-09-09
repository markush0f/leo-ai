use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Grok,
    Gpt,
    Ollama,
    Claude,
}

impl ProviderId {
    pub fn parse(name: &str) -> Result<Self, super::LlmError> {
        match name.trim().to_ascii_lowercase().as_str() {
            "grok" | "xai" | "spacexai" => Ok(Self::Grok),
            "gpt" | "openai" => Ok(Self::Gpt),
            "ollama" => Ok(Self::Ollama),
            "claude" | "anthropic" => Ok(Self::Claude),
            other => Err(super::LlmError::UnknownProvider(other.to_string())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grok => "grok",
            Self::Gpt => "gpt",
            Self::Ollama => "ollama",
            Self::Claude => "claude",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::Grok => "grok-4.6",
            Self::Gpt => "gpt-4.1",
            Self::Ollama => "llama3.2",
            Self::Claude => "claude-sonnet-5",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::Grok => "https://api.x.ai/v1",
            Self::Gpt => "https://api.openai.com/v1",
            Self::Ollama => "http://127.0.0.1:11434",
            Self::Claude => "https://api.anthropic.com/v1",
        }
    }

    pub fn env_key(self) -> Option<&'static str> {
        match self {
            Self::Grok => Some("XAI_API_KEY"),
            Self::Gpt => Some("OPENAI_API_KEY"),
            Self::Ollama => None,
            Self::Claude => Some("ANTHROPIC_API_KEY"),
        }
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl ChatRequest {
    pub fn user(prompt: impl Into<String>) -> Self {
        Self {
            messages: vec![ChatMessage::user(prompt)],
            ..Self::default()
        }
    }

    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.messages.insert(0, ChatMessage::system(system));
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub provider: ProviderId,
    pub model: String,
    pub text: String,
}
