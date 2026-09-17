use std::future::Future;

use leo_llm::{ChatMessage, ChatRequest, ChatResponse, LlmError};

use crate::registry::Registry;

const MAX_ROUNDS: usize = 8;

/// Runs a turn with the real client and resolves requested tool calls.
pub async fn chat(
    client: &leo_llm::Client,
    req: ChatRequest,
    tools: &Registry,
) -> Result<ChatResponse, LlmError> {
    run(|r| client.chat(r), req, tools).await
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
        .find(|m| matches!(m.role, leo_llm::Role::System))
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
    use leo_llm::{ProviderId, ToolCall, ToolSpec};

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
                    .filter(|m| m.role == leo_llm::Role::Tool)
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
                        assert_eq!(last.role, leo_llm::Role::Tool);
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
