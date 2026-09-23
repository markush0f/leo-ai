use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ProviderId, Role, ToolCall};
use crate::{TextSink, stream};

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
        provider_items: Vec::new(),
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

#[derive(Default)]
pub(crate) struct StreamParser {
    sse: stream::Sse,
    text: String,
    model: Option<String>,
    calls: Vec<ToolCall>,
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
        provider: ProviderId,
        fallback_model: &str,
        sink: &TextSink,
    ) -> Result<ChatResponse, LlmError> {
        for data in self.sse.finish() {
            self.event(&data, sink)?;
        }
        if !self.complete {
            return Err(LlmError::IncompleteStream(provider.as_str()));
        }
        self.calls.retain(|call| !call.name.is_empty());
        for (index, call) in self.calls.iter_mut().enumerate() {
            if call.id.is_empty() {
                call.id = format!("call_{index}_{}", call.name);
            }
            if call.arguments.is_empty() {
                call.arguments = "{}".into();
            }
        }
        if self.text.is_empty() && self.calls.is_empty() {
            return Err(LlmError::Empty(provider.as_str()));
        }
        Ok(ChatResponse {
            provider,
            model: self.model.unwrap_or_else(|| fallback_model.to_string()),
            text: self.text,
            tool_calls: self.calls,
            provider_items: Vec::new(),
        })
    }

    fn event(&mut self, data: &[u8], sink: &TextSink) -> Result<(), LlmError> {
        if data == b"[DONE]" {
            self.complete = true;
            return Ok(());
        }
        if data.is_empty() {
            return Ok(());
        }
        let value = stream::json(data)?;
        if let Some(message) = value
            .pointer("/error/message")
            .and_then(serde_json::Value::as_str)
        {
            return Err(LlmError::Http {
                status: 0,
                body: message.to_string(),
            });
        }
        if let Some(model) = value.get("model").and_then(serde_json::Value::as_str) {
            self.model = Some(model.to_string());
        }
        let Some(delta) = value.pointer("/choices/0/delta") else {
            self.mark_finished(&value);
            return Ok(());
        };
        if let Some(text) = delta.get("content").and_then(serde_json::Value::as_str) {
            if !text.is_empty() {
                self.text.push_str(text);
                sink(text.to_string());
            }
        }
        for call in delta
            .get("tool_calls")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let index = call
                .get("index")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as usize;
            while self.calls.len() <= index {
                self.calls.push(ToolCall {
                    id: String::new(),
                    name: String::new(),
                    arguments: String::new(),
                });
            }
            let target = &mut self.calls[index];
            if let Some(id) = call.get("id").and_then(serde_json::Value::as_str) {
                target.id.push_str(id);
            }
            if let Some(name) = call
                .pointer("/function/name")
                .and_then(serde_json::Value::as_str)
            {
                target.name.push_str(name);
            }
            if let Some(args) = call
                .pointer("/function/arguments")
                .and_then(serde_json::Value::as_str)
            {
                target.arguments.push_str(args);
            }
        }
        self.mark_finished(&value);
        Ok(())
    }

    fn mark_finished(&mut self, value: &serde_json::Value) {
        self.complete |= matches!(
            value
                .pointer("/choices/0/finish_reason")
                .and_then(serde_json::Value::as_str),
            Some("stop" | "length" | "tool_calls" | "content_filter" | "function_call")
        );
    }
}

#[cfg(test)]
mod stream_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn streams_fragmented_text_and_tool_arguments() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let copy = output.clone();
        let sink: TextSink = Arc::new(move |text| copy.lock().unwrap().push(text));
        let mut parser = StreamParser::default();
        parser
            .push(
                b"data: {\"model\":\"gpt-x\",\"choices\":[{\"delta\":{\"content\":\"he",
                &sink,
            )
            .unwrap();
        parser.push(b"y\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"echo\",\"arguments\":\"{\\\"x\\\":\"}}]}}]}\n\n", &sink).unwrap();
        parser.push(b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"1}\"}}]}}]}\n\ndata: [DONE]\n\n", &sink).unwrap();
        let response = parser.finish(ProviderId::Gpt, "fallback", &sink).unwrap();
        assert_eq!(response.text, "hey");
        assert_eq!(response.model, "gpt-x");
        assert_eq!(response.tool_calls[0].arguments, r#"{"x":1}"#);
        assert_eq!(*output.lock().unwrap(), ["hey"]);
    }

    #[test]
    fn rejects_eof_without_done_or_finish_reason() {
        let sink: TextSink = Arc::new(|_| {});
        let mut parser = StreamParser::default();
        parser
            .push(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
                &sink,
            )
            .unwrap();
        assert!(matches!(
            parser.finish(ProviderId::Gpt, "gpt", &sink),
            Err(LlmError::IncompleteStream("gpt"))
        ));
    }

    #[test]
    fn accepts_valid_finish_reason_without_done() {
        let sink: TextSink = Arc::new(|_| {});
        let mut parser = StreamParser::default();
        parser
            .push(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\n\n",
                &sink,
            )
            .unwrap();
        assert_eq!(
            parser.finish(ProviderId::Gpt, "gpt", &sink).unwrap().text,
            "ok"
        );
    }
}
