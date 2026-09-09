use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::types::{ChatRequest, ChatResponse, ProviderId, Role};

#[derive(Serialize)]
struct ClaudeRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<ClaudeMessage<'a>>,
}

#[derive(Serialize)]
struct ClaudeMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    model: Option<String>,
    content: Option<Vec<ClaudeBlock>>,
}

#[derive(Deserialize)]
struct ClaudeBlock {
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<String>,
}

pub fn build_body(model: &str, req: &ChatRequest) -> Result<serde_json::Value, LlmError> {
    let system = req
        .messages
        .iter()
        .find(|m| m.role == Role::System)
        .map(|m| m.content.as_str());
    let messages: Vec<ClaudeMessage<'_>> = req
        .messages
        .iter()
        .filter(|m| m.role != Role::System)
        .map(|m| ClaudeMessage {
            role: match m.role {
                Role::Assistant => "assistant",
                _ => "user",
            },
            content: &m.content,
        })
        .collect();

    Ok(serde_json::to_value(ClaudeRequest {
        model,
        max_tokens: req.max_tokens.unwrap_or(1024),
        temperature: req.temperature,
        system,
        messages,
    })?)
}

pub fn parse_response(fallback_model: &str, body: &str) -> Result<ChatResponse, LlmError> {
    let parsed: ClaudeResponse = serde_json::from_str(body)?;
    let text = parsed
        .content
        .unwrap_or_default()
        .into_iter()
        .filter(|b| b.kind.as_deref().unwrap_or("text") == "text")
        .filter_map(|b| b.text)
        .collect::<Vec<_>>()
        .join("");
    if text.is_empty() {
        return Err(LlmError::Empty(ProviderId::Claude.as_str()));
    }
    Ok(ChatResponse {
        provider: ProviderId::Claude,
        model: parsed
            .model
            .unwrap_or_else(|| fallback_model.to_string()),
        text,
    })
}
