use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ProviderId};

#[derive(Serialize)]
struct OllamaChat<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    model: Option<String>,
    message: Option<OllamaMessage>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct OllamaMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Tags {
    models: Vec<Tag>,
}

#[derive(Deserialize)]
struct Tag {
    name: String,
}

pub fn origin(base: &str) -> String {
    let b = base.trim().trim_end_matches('/');
    b.strip_suffix("/v1").unwrap_or(b).trim_end_matches('/').to_string()
}

pub fn build_body(model: &str, req: &ChatRequest) -> Result<serde_json::Value, LlmError> {
    let options = match (req.temperature, req.max_tokens) {
        (None, None) => None,
        (temperature, max_tokens) => Some(OllamaOptions {
            temperature,
            num_predict: max_tokens,
        }),
    };
    Ok(serde_json::to_value(OllamaChat {
        model,
        messages: &req.messages,
        stream: false,
        options,
    })?)
}

pub fn parse_response(fallback_model: &str, body: &str) -> Result<ChatResponse, LlmError> {
    let parsed: OllamaChatResponse = serde_json::from_str(body)?;
    if let Some(err) = parsed.error.filter(|s| !s.is_empty()) {
        return Err(LlmError::Http {
            status: 0,
            body: err,
        });
    }
    let text = parsed
        .message
        .and_then(|m| m.content)
        .filter(|s| !s.is_empty())
        .ok_or(LlmError::Empty(ProviderId::Ollama.as_str()))?;
    Ok(ChatResponse {
        provider: ProviderId::Ollama,
        model: parsed
            .model
            .unwrap_or_else(|| fallback_model.to_string()),
        text,
    })
}

pub fn parse_tags(body: &str) -> Result<Vec<String>, LlmError> {
    let tags: Tags = serde_json::from_str(body)?;
    Ok(tags.models.into_iter().map(|t| t.name).collect())
}
