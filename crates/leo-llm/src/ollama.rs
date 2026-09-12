use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ProviderId, Role, ToolCall};

#[derive(Serialize)]
struct OllamaChat<'a> {
    model: &'a str,
    messages: Vec<serde_json::Value>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OllamaTool<'a>>,
}

#[derive(Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Serialize)]
struct OllamaTool<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OllamaFunction<'a>,
}

#[derive(Serialize)]
struct OllamaFunction<'a> {
    name: &'a str,
    description: &'a str,
    parameters: &'a serde_json::Value,
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
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Deserialize)]
struct OllamaToolCall {
    function: Option<OllamaFnCall>,
}

#[derive(Deserialize)]
struct OllamaFnCall {
    name: Option<String>,
    arguments: Option<serde_json::Value>,
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
    b.strip_suffix("/v1")
        .unwrap_or(b)
        .trim_end_matches('/')
        .to_string()
}

pub fn build_body(model: &str, req: &ChatRequest) -> Result<serde_json::Value, LlmError> {
    let options = match (req.temperature, req.max_tokens) {
        (None, None) => None,
        (temperature, max_tokens) => Some(OllamaOptions {
            temperature,
            num_predict: max_tokens,
        }),
    };
    let tools: Vec<OllamaTool<'_>> = req
        .tools
        .iter()
        .map(|t| OllamaTool {
            kind: "function",
            function: OllamaFunction {
                name: &t.name,
                description: &t.description,
                parameters: &t.parameters,
            },
        })
        .collect();
    Ok(serde_json::to_value(OllamaChat {
        model,
        messages: wire_messages(&req.messages),
        stream: false,
        options,
        tools,
    })?)
}

fn wire_messages(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
    messages.iter().map(wire_message).collect()
}

fn wire_message(m: &ChatMessage) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "role".into(),
        serde_json::Value::String(
            match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            }
            .into(),
        ),
    );
    obj.insert("content".into(), m.content.clone().into());
    if !m.tool_calls.is_empty() {
        let calls: Vec<serde_json::Value> = m
            .tool_calls
            .iter()
            .map(|c| {
                let args: serde_json::Value =
                    serde_json::from_str(&c.arguments).unwrap_or(serde_json::json!({}));
                serde_json::json!({
                    "function": {
                        "name": c.name,
                        "arguments": args,
                    }
                })
            })
            .collect();
        obj.insert("tool_calls".into(), serde_json::Value::Array(calls));
    }
    serde_json::Value::Object(obj)
}

pub fn parse_response(fallback_model: &str, body: &str) -> Result<ChatResponse, LlmError> {
    let parsed: OllamaChatResponse = serde_json::from_str(body)?;
    if let Some(err) = parsed.error.filter(|s| !s.is_empty()) {
        return Err(LlmError::Http {
            status: 0,
            body: err,
        });
    }
    let tool_calls = parsed
        .message
        .as_ref()
        .map(parse_tool_calls)
        .unwrap_or_default();
    let text = parsed.message.and_then(|m| m.content).unwrap_or_default();
    if text.is_empty() && tool_calls.is_empty() {
        return Err(LlmError::Empty(ProviderId::Ollama.as_str()));
    }
    Ok(ChatResponse {
        provider: ProviderId::Ollama,
        model: parsed.model.unwrap_or_else(|| fallback_model.to_string()),
        text,
        tool_calls,
    })
}

fn parse_tool_calls(message: &OllamaMessage) -> Vec<ToolCall> {
    let Some(calls) = &message.tool_calls else {
        return Vec::new();
    };
    calls
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let fn_call = c.function.as_ref()?;
            let name = fn_call.name.clone().filter(|s| !s.is_empty())?;
            let arguments = match &fn_call.arguments {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => "{}".into(),
            };
            Some(ToolCall {
                id: format!("call_{i}_{name}"),
                name,
                arguments,
            })
        })
        .collect()
}

pub fn parse_tags(body: &str) -> Result<Vec<String>, LlmError> {
    let tags: Tags = serde_json::from_str(body)?;
    Ok(tags.models.into_iter().map(|t| t.name).collect())
}
