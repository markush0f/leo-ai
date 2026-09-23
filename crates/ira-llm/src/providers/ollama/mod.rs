use serde::{Deserialize, Serialize};

use crate::error::LlmError;
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ProviderId, Role, ToolCall};
use crate::{TextSink, stream};

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
    #[serde(default)]
    done: bool,
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

pub(crate) fn build_stream_body(
    model: &str,
    req: &ChatRequest,
) -> Result<serde_json::Value, LlmError> {
    let mut body = build_body(model, req)?;
    body["stream"] = true.into();
    Ok(body)
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
        provider_items: Vec::new(),
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

#[derive(Default)]
pub(crate) struct StreamParser {
    lines: stream::Lines,
    text: String,
    model: Option<String>,
    calls: Vec<ToolCall>,
    complete: bool,
}

impl StreamParser {
    pub(crate) fn push(&mut self, chunk: &[u8], sink: &TextSink) -> Result<(), LlmError> {
        for line in self.lines.push(chunk) {
            self.line(&line, sink)?;
        }
        Ok(())
    }

    pub(crate) fn finish(
        mut self,
        fallback_model: &str,
        sink: &TextSink,
    ) -> Result<ChatResponse, LlmError> {
        if let Some(line) = self.lines.finish() {
            self.line(&line, sink)?;
        }
        if !self.complete {
            return Err(LlmError::IncompleteStream(ProviderId::Ollama.as_str()));
        }
        if self.text.is_empty() && self.calls.is_empty() {
            return Err(LlmError::Empty(ProviderId::Ollama.as_str()));
        }
        Ok(ChatResponse {
            provider: ProviderId::Ollama,
            model: self.model.unwrap_or_else(|| fallback_model.to_string()),
            text: self.text,
            tool_calls: self.calls,
            provider_items: Vec::new(),
        })
    }

    fn line(&mut self, line: &[u8], sink: &TextSink) -> Result<(), LlmError> {
        if line.is_empty() {
            return Ok(());
        }
        let parsed: OllamaChatResponse = serde_json::from_slice(line)?;
        if let Some(error) = parsed.error.filter(|value| !value.is_empty()) {
            return Err(LlmError::Http {
                status: 0,
                body: error,
            });
        }
        if let Some(model) = parsed.model {
            self.model = Some(model);
        }
        if let Some(message) = parsed.message {
            for call in parse_tool_calls(&message) {
                if let Some(existing) = self.calls.iter_mut().find(|known| known.id == call.id) {
                    *existing = call;
                } else {
                    self.calls.push(call);
                }
            }
            if let Some(text) = message.content.filter(|value| !value.is_empty()) {
                self.text.push_str(&text);
                sink(text);
            }
        }
        self.complete |= parsed.done;
        Ok(())
    }
}

pub fn parse_tags(body: &str) -> Result<Vec<String>, LlmError> {
    let tags: Tags = serde_json::from_str(body)?;
    Ok(tags.models.into_iter().map(|t| t.name).collect())
}

#[cfg(test)]
mod stream_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn streams_fragmented_ndjson() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let copy = output.clone();
        let sink: TextSink = Arc::new(move |text| copy.lock().unwrap().push(text));
        let mut parser = StreamParser::default();
        parser
            .push(b"{\"model\":\"llama\",\"message\":{\"content\":\"ho", &sink)
            .unwrap();
        parser.push(b"la\"}}\n{\"message\":{\"content\":\"!\",\"tool_calls\":[{\"function\":{\"name\":\"echo\",\"arguments\":{\"x\":1}}}]},\"done\":true}\n", &sink).unwrap();
        let response = parser.finish("fallback", &sink).unwrap();
        assert_eq!(response.text, "hola!");
        assert_eq!(response.tool_calls[0].name, "echo");
        assert_eq!(*output.lock().unwrap(), ["hola", "!"]);
    }

    #[test]
    fn rejects_eof_without_done() {
        let sink: TextSink = Arc::new(|_| {});
        let mut parser = StreamParser::default();
        parser
            .push(b"{\"message\":{\"content\":\"partial\"}}\n", &sink)
            .unwrap();
        assert!(matches!(
            parser.finish("llama", &sink),
            Err(LlmError::IncompleteStream("ollama"))
        ));
    }
}
