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
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Model-visible function description; `parameters` contains its JSON Schema.
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Model request; `arguments` preserves the provider's serialized JSON arguments.
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: Vec::new(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: Vec::new(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls: Vec::new(),
        }
    }

    pub fn assistant_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_call_id: None,
            name: None,
            tool_calls,
        }
    }

    pub fn tool(
        id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_call_id: Some(id.into()),
            name: Some(name.into()),
            tool_calls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
/// Provider-independent request; `model: None` uses the client's default model.
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Vec<ToolSpec>,
    /// Grok 4.5/4.6: `low` | `medium` | `high` | `xhigh`; reasoning cannot be fully disabled.
    pub reasoning_effort: Option<String>,
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

    pub fn with_history(system: &str, history: impl IntoIterator<Item = ChatMessage>) -> Self {
        let mut messages = Vec::new();
        if !system.is_empty() {
            messages.push(ChatMessage::system(system));
        }
        messages.extend(history);
        Self {
            messages,
            ..Self::default()
        }
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = tools;
        self
    }
}

#[derive(Debug, Clone)]
/// Normalized response containing text, tool calls, or both.
pub struct ChatResponse {
    pub provider: ProviderId,
    pub model: String,
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

impl ChatResponse {
    pub fn new(provider: ProviderId, model: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
            text: text.into(),
            tool_calls: Vec::new(),
        }
    }
}
