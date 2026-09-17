use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ProviderId, Role, ToolCall};

#[derive(Serialize)]
struct ClaudeRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ClaudeTool<'a>>,
}

#[derive(Serialize)]
struct ClaudeTool<'a> {
    name: &'a str,
    description: &'a str,
    input_schema: &'a serde_json::Value,
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
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

pub fn build_body(model: &str, req: &ChatRequest) -> Result<serde_json::Value, LlmError> {
    let system = req
        .messages
        .iter()
        .find(|m| m.role == Role::System)
        .map(|m| m.content.as_str());
    let tools: Vec<ClaudeTool<'_>> = req
        .tools
        .iter()
        .map(|t| ClaudeTool {
            name: &t.name,
            description: &t.description,
            input_schema: &t.parameters,
        })
        .collect();

    Ok(serde_json::to_value(ClaudeRequest {
        model,
        max_tokens: req.max_tokens.unwrap_or(1024),
        temperature: req.temperature,
        system,
        messages: wire_messages(&req.messages),
        tools,
    })?)
}

fn wire_messages(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        let m = &messages[i];
        if m.role == Role::System {
            i += 1;
            continue;
        }
        if m.role == Role::Tool {
            let mut results = Vec::new();
            while i < messages.len() && messages[i].role == Role::Tool {
                let t = &messages[i];
                results.push(serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": t.tool_call_id.as_deref().unwrap_or(""),
                    "content": t.content,
                }));
                i += 1;
            }
            out.push(serde_json::json!({
                "role": "user",
                "content": results,
            }));
            continue;
        }
        if m.role == Role::Assistant && !m.tool_calls.is_empty() {
            let mut blocks = Vec::new();
            if !m.content.is_empty() {
                blocks.push(serde_json::json!({"type": "text", "text": m.content}));
            }
            for call in &m.tool_calls {
                let input: serde_json::Value =
                    serde_json::from_str(&call.arguments).unwrap_or(serde_json::json!({}));
                blocks.push(serde_json::json!({
                    "type": "tool_use",
                    "id": call.id,
                    "name": call.name,
                    "input": input,
                }));
            }
            out.push(serde_json::json!({
                "role": "assistant",
                "content": blocks,
            }));
            i += 1;
            continue;
        }
        out.push(serde_json::json!({
            "role": match m.role {
                Role::Assistant => "assistant",
                _ => "user",
            },
            "content": m.content,
        }));
        i += 1;
    }
    out
}

pub fn parse_response(fallback_model: &str, body: &str) -> Result<ChatResponse, LlmError> {
    let parsed: ClaudeResponse = serde_json::from_str(body)?;
    let blocks = parsed.content.unwrap_or_default();
    let text = blocks
        .iter()
        .filter(|b| b.kind.as_deref().unwrap_or("text") == "text")
        .filter_map(|b| b.text.clone())
        .collect::<Vec<_>>()
        .join("");
    let tool_calls = blocks
        .iter()
        .filter(|b| b.kind.as_deref() == Some("tool_use"))
        .enumerate()
        .filter_map(|(i, b)| {
            let name = b.name.clone().filter(|s| !s.is_empty())?;
            Some(ToolCall {
                id: b
                    .id
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("call_{i}_{name}")),
                name,
                arguments: b
                    .input
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "{}".into()),
            })
        })
        .collect::<Vec<_>>();
    if text.is_empty() && tool_calls.is_empty() {
        return Err(LlmError::Empty(ProviderId::Claude.as_str()));
    }
    Ok(ChatResponse {
        provider: ProviderId::Claude,
        model: parsed.model.unwrap_or_else(|| fallback_model.to_string()),
        text,
        tool_calls,
        provider_items: Vec::new(),
    })
}
