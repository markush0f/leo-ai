use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ProviderId, Role, ToolCall};

#[derive(Serialize)]
struct OpenAiRequest<'a> {
    model: &'a str,
    messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAiTool<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
}

#[derive(Serialize)]
struct OpenAiTool<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    function: OpenAiFunction<'a>,
}

#[derive(Serialize)]
struct OpenAiFunction<'a> {
    name: &'a str,
    description: &'a str,
    parameters: &'a serde_json::Value,
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
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Deserialize)]
struct OpenAiToolCall {
    id: Option<String>,
    function: Option<OpenAiFnCall>,
}

#[derive(Deserialize)]
struct OpenAiFnCall {
    name: Option<String>,
    arguments: Option<String>,
}

pub fn build_body(model: &str, req: &ChatRequest) -> Result<serde_json::Value, LlmError> {
    let tools: Vec<OpenAiTool<'_>> = req
        .tools
        .iter()
        .map(|t| OpenAiTool {
            kind: "function",
            function: OpenAiFunction {
                name: &t.name,
                description: &t.description,
                parameters: &t.parameters,
            },
        })
        .collect();
    Ok(serde_json::to_value(OpenAiRequest {
        model,
        messages: wire_messages(&req.messages),
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        tools,
        reasoning_effort: req.reasoning_effort.as_deref(),
    })?)
}

fn wire_messages(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
    messages.iter().map(wire_message).collect()
}

fn wire_message(m: &ChatMessage) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "role".into(),
        serde_json::Value::String(role_str(m.role).into()),
    );
    match m.role {
        Role::Tool => {
            if let Some(id) = &m.tool_call_id {
                obj.insert("tool_call_id".into(), id.clone().into());
            }
            obj.insert("content".into(), m.content.clone().into());
        }
        Role::Assistant if !m.tool_calls.is_empty() => {
            if !m.content.is_empty() {
                obj.insert("content".into(), m.content.clone().into());
            }
            obj.insert(
                "tool_calls".into(),
                serde_json::Value::Array(m.tool_calls.iter().map(wire_tool_call).collect()),
            );
        }
        _ => {
            obj.insert("content".into(), m.content.clone().into());
        }
    }
    serde_json::Value::Object(obj)
}

fn wire_tool_call(call: &ToolCall) -> serde_json::Value {
    serde_json::json!({
        "id": call.id,
        "type": "function",
        "function": {
            "name": call.name,
            "arguments": call.arguments,
        }
    })
}

fn role_str(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

pub fn parse_response(
    provider: ProviderId,
    fallback_model: &str,
    body: &str,
) -> Result<ChatResponse, LlmError> {
    let parsed: OpenAiResponse = serde_json::from_str(body)?;
    let message = parsed
        .choices
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.message.as_ref());
    let tool_calls = message.map(parse_tool_calls).unwrap_or_default();
    let text = message.and_then(|m| m.content.clone()).unwrap_or_default();
    if text.is_empty() && tool_calls.is_empty() {
        return Err(LlmError::Empty(provider.as_str()));
    }
    Ok(ChatResponse {
        provider,
        model: parsed.model.unwrap_or_else(|| fallback_model.to_string()),
        text,
        tool_calls,
    })
}

fn parse_tool_calls(message: &OpenAiMessage) -> Vec<ToolCall> {
    let Some(calls) = &message.tool_calls else {
        return Vec::new();
    };
    calls
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let fn_call = c.function.as_ref()?;
            let name = fn_call.name.clone().filter(|s| !s.is_empty())?;
            Some(ToolCall {
                id: c
                    .id
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("call_{i}_{name}")),
                name,
                arguments: fn_call.arguments.clone().unwrap_or_else(|| "{}".into()),
            })
        })
        .collect()
}
