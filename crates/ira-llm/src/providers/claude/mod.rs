use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ProviderId, Role, ToolCall};
use crate::{TextSink, stream};

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

pub(crate) fn build_stream_body(
    model: &str,
    req: &ChatRequest,
) -> Result<serde_json::Value, LlmError> {
    let mut body = build_body(model, req)?;
    body["stream"] = true.into();
    Ok(body)
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

#[derive(Default)]
pub(crate) struct StreamParser {
    sse: stream::Sse,
    text: String,
    model: Option<String>,
    calls: Vec<(usize, ToolCall)>,
    complete: bool,
}

impl StreamParser {
    pub(crate) fn push(&mut self, chunk: &[u8], sink: &TextSink) -> Result<(), LlmError> {
        for data in self.sse.push(chunk) {
            self.event(&data, sink)?;
        }
        Ok(())
    }

    pub(crate) fn finish(
        mut self,
        fallback_model: &str,
        sink: &TextSink,
    ) -> Result<ChatResponse, LlmError> {
        for data in self.sse.finish() {
            self.event(&data, sink)?;
        }
        if !self.complete {
            return Err(LlmError::IncompleteStream(ProviderId::Claude.as_str()));
        }
        self.calls.retain(|(_, call)| !call.name.is_empty());
        for (index, (_, call)) in self.calls.iter_mut().enumerate() {
            if call.id.is_empty() {
                call.id = format!("call_{index}_{}", call.name);
            }
            if call.arguments.is_empty() {
                call.arguments = "{}".into();
            }
        }
        if self.text.is_empty() && self.calls.is_empty() {
            return Err(LlmError::Empty(ProviderId::Claude.as_str()));
        }
        Ok(ChatResponse {
            provider: ProviderId::Claude,
            model: self.model.unwrap_or_else(|| fallback_model.to_string()),
            text: self.text,
            tool_calls: self.calls.into_iter().map(|(_, call)| call).collect(),
            provider_items: Vec::new(),
        })
    }

    fn event(&mut self, data: &[u8], sink: &TextSink) -> Result<(), LlmError> {
        let value = stream::json(data)?;
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("message_start") => {
                if let Some(model) = value
                    .pointer("/message/model")
                    .and_then(serde_json::Value::as_str)
                {
                    self.model = Some(model.to_string());
                }
            }
            Some("content_block_start") => {
                let Some(block) = value.get("content_block") else {
                    return Ok(());
                };
                if block.get("type").and_then(serde_json::Value::as_str) == Some("tool_use") {
                    let index = value
                        .get("index")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as usize;
                    let call = ToolCall {
                        id: block
                            .get("id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        name: block
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        arguments: String::new(),
                    };
                    if let Some((_, existing)) =
                        self.calls.iter_mut().find(|(block, _)| *block == index)
                    {
                        existing.id = call.id;
                        existing.name = call.name;
                    } else {
                        self.calls.push((index, call));
                    }
                }
            }
            Some("content_block_delta") => {
                let delta = value.get("delta").unwrap_or(&serde_json::Value::Null);
                if let Some(text) = delta.get("text").and_then(serde_json::Value::as_str) {
                    if !text.is_empty() {
                        self.text.push_str(text);
                        sink(text.to_string());
                    }
                }
                if let Some(json) = delta
                    .get("partial_json")
                    .and_then(serde_json::Value::as_str)
                {
                    let index = value
                        .get("index")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as usize;
                    if let Some((_, call)) =
                        self.calls.iter_mut().find(|(block, _)| *block == index)
                    {
                        call.arguments.push_str(json);
                    }
                }
            }
            Some("error") => {
                return Err(LlmError::Http {
                    status: 0,
                    body: value
                        .pointer("/error/message")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Claude stream error")
                        .to_string(),
                });
            }
            Some("message_stop") => self.complete = true,
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod stream_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn streams_text_and_accumulates_tool_json() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let copy = output.clone();
        let sink: TextSink = Arc::new(move |text| copy.lock().unwrap().push(text));
        let body = concat!(
            "data: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-x\"}}\n\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
            "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"echo\",\"input\":{}}}\n\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"x\\\":1}\"}}\n\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );
        let mut parser = StreamParser::default();
        for chunk in body.as_bytes().chunks(7) {
            parser.push(chunk, &sink).unwrap();
        }
        let response = parser.finish("fallback", &sink).unwrap();
        assert_eq!(response.text, "hi");
        assert_eq!(response.model, "claude-x");
        assert_eq!(response.tool_calls[0].arguments, r#"{"x":1}"#);
        assert_eq!(*output.lock().unwrap(), ["hi"]);
    }

    #[test]
    fn rejects_eof_without_message_stop() {
        let sink: TextSink = Arc::new(|_| {});
        let mut parser = StreamParser::default();
        parser
            .push(
                b"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"partial\"}}\n\n",
                &sink,
            )
            .unwrap();
        assert!(matches!(
            parser.finish("claude", &sink),
            Err(LlmError::IncompleteStream("claude"))
        ));
    }

    #[test]
    fn repeated_tool_start_does_not_duplicate_or_reset_arguments() {
        let sink: TextSink = Arc::new(|_| {});
        let start = b"data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"echo\",\"input\":{}}}\n\n";
        let mut parser = StreamParser::default();
        parser.push(start, &sink).unwrap();
        parser
            .push(
                b"data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{}\"}}\n\n",
                &sink,
            )
            .unwrap();
        parser.push(start, &sink).unwrap();
        parser
            .push(b"data: {\"type\":\"message_stop\"}\n\n", &sink)
            .unwrap();
        let response = parser.finish("claude", &sink).unwrap();
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].arguments, "{}");
    }
}
