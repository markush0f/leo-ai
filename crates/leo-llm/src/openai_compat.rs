use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ProviderId};

#[derive(Serialize)]
struct OpenAiRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    model: Option<String>,
    choices: Option<Vec<OpenAiChoice>>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: Option<OpenAiMessage>,
}

#[derive(Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
}

pub fn build_body(model: &str, req: &ChatRequest) -> Result<serde_json::Value, LlmError> {
    Ok(serde_json::to_value(OpenAiRequest {
        model,
        messages: &req.messages,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
    })?)
}

pub fn parse_response(
    provider: ProviderId,
    fallback_model: &str,
    body: &str,
) -> Result<ChatResponse, LlmError> {
    let parsed: OpenAiResponse = serde_json::from_str(body)?;
    let text = parsed
        .choices
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.message.as_ref())
        .and_then(|m| m.content.clone())
        .filter(|s| !s.is_empty())
        .ok_or(LlmError::Empty(provider.as_str()))?;
    Ok(ChatResponse {
        provider,
        model: parsed
            .model
            .unwrap_or_else(|| fallback_model.to_string()),
        text,
    })
}
