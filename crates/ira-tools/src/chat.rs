use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ira_llm::{ChatMessage, ChatRequest, ChatResponse, LlmError, TextSink};

use crate::registry::Registry;

const MAX_ROUNDS: usize = 8;

/// Event emitted by a streaming tool-enabled chat turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// Provider-generated text fragment.
    Delta(String),
    /// Discard provisional text because the model requested tools.
    Reset,
}

/// Callback invoked for streaming chat events.
pub type StreamSink = Arc<dyn Fn(StreamEvent) + Send + Sync + 'static>;

/// Runs a turn with the real client and resolves requested tool calls.
pub async fn chat(
    client: &ira_llm::Client,
    req: ChatRequest,
    tools: &Registry,
) -> Result<ChatResponse, LlmError> {
    run(|r| client.chat(r), req, tools).await
}

/// Streams a turn while preserving the same tool-call loop as [`chat`].
pub async fn chat_stream(
    client: &ira_llm::Client,
    req: ChatRequest,
    tools: &Registry,
    sink: StreamSink,
) -> Result<ChatResponse, LlmError> {
    run_stream(
        |request, text| client.chat_stream(request, text),
        req,
        tools,
        sink,
    )
    .await
}

async fn run_stream<F, Fut>(
    mut chat: F,
    mut req: ChatRequest,
    tools: &Registry,
    sink: StreamSink,
) -> Result<ChatResponse, LlmError>
where
    F: FnMut(ChatRequest, TextSink) -> Fut,
    Fut: Future<Output = Result<ChatResponse, LlmError>>,
{
    if !tools.is_empty() {
        req.tools = tools.specs();
    }
    if req.tools.is_empty() {
        let output = sink.clone();
        let text: TextSink = Arc::new(move |delta| output(StreamEvent::Delta(delta)));
        return chat(req, text).await;
    }
    if req.max_tokens.is_none() {
        req.max_tokens = Some(4096);
    }
    hint_system(&mut req, tools);

    for _ in 0..MAX_ROUNDS {
        let emitted = Arc::new(AtomicBool::new(false));
        let marked = emitted.clone();
        let output = sink.clone();
        let text: TextSink = Arc::new(move |delta| {
            if !delta.is_empty() {
                marked.store(true, Ordering::Relaxed);
                output(StreamEvent::Delta(delta));
            }
        });
        let resp = chat(req.clone(), text).await?;
        if resp.tool_calls.is_empty() {
            return Ok(resp);
        }
        if emitted.load(Ordering::Relaxed) {
            sink(StreamEvent::Reset);
        }
        req.messages
            .push(ChatMessage::assistant_tools_with_provider_items(
                resp.text.clone(),
                resp.tool_calls.clone(),
                resp.provider_items.clone(),
            ));
        for call in &resp.tool_calls {
            let args = parse_args(&call.arguments);
            tracing::info!(tool = %call.name, "tool");
            let result = tools.call(&call.name, args).await;
            req.messages
                .push(ChatMessage::tool(&call.id, &call.name, result));
        }
    }
    Err(LlmError::ToolLoop)
}

/// Chat loop with an injectable transport, also suitable for offline tests.
///
/// Executes tool calls in order and preserves their IDs in follow-up history.
/// Eight responses that still request tools produce `LlmError::ToolLoop`.
/// LLM failures abort the turn; tool failures return to the model as JSON content.
pub async fn run<F, Fut>(
    mut chat: F,
    mut req: ChatRequest,
    tools: &Registry,
) -> Result<ChatResponse, LlmError>
where
    F: FnMut(ChatRequest) -> Fut,
    Fut: Future<Output = Result<ChatResponse, LlmError>>,
{
    if !tools.is_empty() {
        req.tools = tools.specs();
    }
    if req.tools.is_empty() {
        return chat(req).await;
    }
    if req.max_tokens.is_none() {
        req.max_tokens = Some(4096);
    }
    hint_system(&mut req, tools);

    for _ in 0..MAX_ROUNDS {
        let resp = chat(req.clone()).await?;
        if resp.tool_calls.is_empty() {
            return Ok(resp);
        }
        req.messages
            .push(ChatMessage::assistant_tools_with_provider_items(
                resp.text.clone(),
                resp.tool_calls.clone(),
                resp.provider_items.clone(),
            ));
        for call in &resp.tool_calls {
            let args = parse_args(&call.arguments);
            tracing::info!(tool = %call.name, "tool");
            let result = tools.call(&call.name, args).await;
            req.messages
                .push(ChatMessage::tool(&call.id, &call.name, result));
        }
    }
    Err(LlmError::ToolLoop)
}

fn hint_system(req: &mut ChatRequest, tools: &Registry) {
    let names = tools.names();
    if names.is_empty() {
        return;
    }
    let hint = format!("Puedes usar estas herramientas: {}.", names.join(", "));
    if let Some(sys) = req
        .messages
        .iter_mut()
        .find(|m| matches!(m.role, ira_llm::Role::System))
    {
        if names.iter().any(|n| sys.content.contains(n)) {
            return;
        }
        if !sys.content.is_empty() {
            sys.content.push_str("\n\n");
        }
        sys.content.push_str(&hint);
    } else {
        req.messages.insert(0, ChatMessage::system(hint));
    }
}

fn parse_args(raw: &str) -> serde_json::Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return serde_json::json!({});
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;
    use crate::error::stringify;
    use crate::tool::DynTool;
    use ira_llm::{ProviderId, ToolCall, ToolSpec};

    fn dummy_registry() -> Registry {
        let mut b = Registry::builder(Context::default());
        b.add(DynTool::new(
            ToolSpec {
                name: "echo".into(),
                description: "eco".into(),
                parameters: serde_json::json!({"type": "object", "properties": {}}),
            },
            |_ctx, args| async move {
                stringify(Ok::<_, String>(serde_json::json!({
                    "ok": true,
                    "echo": args,
                })))
            },
        ));
        b.build()
    }

    #[tokio::test]
    async fn empty_registry_is_passthrough() {
        let tools = Registry::default();
        let req = ChatRequest::user("hola");
        let resp = run(
            |_| async { Ok(ChatResponse::new(ProviderId::Grok, "grok-4.6", "hey")) },
            req,
            &tools,
        )
        .await
        .unwrap();
        assert_eq!(resp.text, "hey");
    }

    #[tokio::test]
    async fn executes_tool_then_returns_final_text() {
        let tools = dummy_registry();
        let req = ChatRequest::user("eco");
        let resp = run(
            move |incoming| {
                let n = incoming
                    .messages
                    .iter()
                    .filter(|m| m.role == ira_llm::Role::Tool)
                    .count();
                async move {
                    if n == 0 {
                        Ok(ChatResponse {
                            provider: ProviderId::Grok,
                            model: "grok-4.6".into(),
                            text: String::new(),
                            tool_calls: vec![ToolCall {
                                id: "call_1".into(),
                                name: "echo".into(),
                                arguments: r#"{"msg":"hola"}"#.into(),
                            }],
                            provider_items: Vec::new(),
                        })
                    } else {
                        let last = incoming.messages.last().unwrap();
                        assert_eq!(last.role, ira_llm::Role::Tool);
                        assert!(last.content.contains("hola"));
                        Ok(ChatResponse::new(ProviderId::Grok, "grok-4.6", "listo"))
                    }
                }
            },
            req,
            &tools,
        )
        .await
        .unwrap();
        assert_eq!(resp.text, "listo");
    }

    #[tokio::test]
    async fn streaming_resets_provisional_tool_round() {
        use std::sync::Mutex;

        let tools = dummy_registry();
        let events = Arc::new(Mutex::new(Vec::new()));
        let recorded = events.clone();
        let sink: StreamSink = Arc::new(move |event| recorded.lock().unwrap().push(event));
        let resp = run_stream(
            move |incoming, text| {
                let has_result = incoming
                    .messages
                    .iter()
                    .any(|m| m.role == ira_llm::Role::Tool);
                async move {
                    if has_result {
                        text("final".into());
                        Ok(ChatResponse::new(ProviderId::Grok, "grok-4.6", "final"))
                    } else {
                        text("checking".into());
                        Ok(ChatResponse {
                            provider: ProviderId::Grok,
                            model: "grok-4.6".into(),
                            text: "checking".into(),
                            tool_calls: vec![ToolCall {
                                id: "call_1".into(),
                                name: "echo".into(),
                                arguments: r#"{"msg":"hola"}"#.into(),
                            }],
                            provider_items: Vec::new(),
                        })
                    }
                }
            },
            ChatRequest::user("eco"),
            &tools,
            sink,
        )
        .await
        .unwrap();
        assert_eq!(resp.text, "final");
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                StreamEvent::Delta("checking".into()),
                StreamEvent::Reset,
                StreamEvent::Delta("final".into()),
            ]
        );
    }

    #[test]
    fn from_env_registers_local_tools() {
        let tools = Registry::from_env();
        let names = tools.names();
        assert!(names.contains(&"read_file".into()));
        assert!(names.contains(&"execute_command".into()));
        assert!(names.contains(&"get_weather".into()));
        assert!(names.contains(&"list_processes".into()));
    }
}
