mod appflowy;
mod markdown;

use std::future::Future;

use leo_llm::{ChatMessage, ChatRequest, ChatResponse, LlmError, ToolSpec};

const MAX_ROUNDS: usize = 8;

#[derive(Clone, Default)]
pub struct Registry {
    appflowy: Option<appflowy::Client>,
}

impl Registry {
    pub fn from_env() -> Self {
        let appflowy = appflowy::Config::from_env().map(appflowy::Client::new);
        if appflowy.is_some() {
            tracing::info!("tool appflowy_write listo");
        }
        Self { appflowy }
    }

    pub fn is_empty(&self) -> bool {
        self.appflowy.is_none()
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        let mut out = Vec::new();
        if self.appflowy.is_some() {
            out.push(appflowy::spec());
        }
        out
    }

    pub async fn call(&self, name: &str, args: serde_json::Value) -> String {
        match name {
            "appflowy_write" => match &self.appflowy {
                Some(client) => appflowy::call(client, args).await,
                None => "error: AppFlowy no está configurado".into(),
            },
            other => format!("error: herramienta desconocida: {other}"),
        }
    }
}

pub async fn chat(
    client: &leo_llm::Client,
    req: ChatRequest,
    tools: &Registry,
) -> Result<ChatResponse, LlmError> {
    run(|r| client.chat(r), req, tools).await
}

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
    hint_system(&mut req);

    for _ in 0..MAX_ROUNDS {
        let resp = chat(req.clone()).await?;
        if resp.tool_calls.is_empty() {
            return Ok(resp);
        }
        req.messages.push(ChatMessage::assistant_tools(
            resp.text.clone(),
            resp.tool_calls.clone(),
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

fn hint_system(req: &mut ChatRequest) {
    const HINT: &str = "Puedes guardar notas en AppFlowy con la herramienta appflowy_write.";
    if let Some(sys) = req.messages.iter_mut().find(|m| {
        matches!(m.role, leo_llm::Role::System)
    }) {
        if !sys.content.contains("appflowy_write") {
            if !sys.content.is_empty() {
                sys.content.push_str("\n\n");
            }
            sys.content.push_str(HINT);
        }
    } else {
        req.messages.insert(0, ChatMessage::system(HINT));
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
    use leo_llm::{ProviderId, ToolCall};
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn empty_registry_is_passthrough() {
        let tools = Registry::default();
        let req = ChatRequest::user("hola");
        let resp = run(
            |_| async {
                Ok(ChatResponse::new(ProviderId::Grok, "grok-4.6", "hey"))
            },
            req,
            &tools,
        )
        .await
        .unwrap();
        assert_eq!(resp.text, "hey");
    }

    #[tokio::test]
    async fn executes_tool_then_returns_final_text() {
        let calls = Arc::new(Mutex::new(0u8));
        let c = Arc::clone(&calls);
        let req = ChatRequest::user("guarda una nota").with_tools(vec![appflowy::spec()]);
        let resp = run(
            move |incoming| {
                let n = {
                    let mut g = c.lock().unwrap();
                    *g += 1;
                    *g
                };
                let specs = incoming.tools.clone();
                async move {
                    assert!(!specs.is_empty());
                    if n == 1 {
                        Ok(ChatResponse {
                            provider: ProviderId::Grok,
                            model: "grok-4.6".into(),
                            text: String::new(),
                            tool_calls: vec![ToolCall {
                                id: "call_1".into(),
                                name: "appflowy_write".into(),
                                arguments: r#"{"title":"Nota","markdown":"hola"}"#.into(),
                            }],
                        })
                    } else {
                        let last = incoming.messages.last().unwrap();
                        assert_eq!(last.role, leo_llm::Role::Tool);
                        assert!(last.content.contains("no está configurado"));
                        Ok(ChatResponse::new(ProviderId::Grok, "grok-4.6", "listo"))
                    }
                }
            },
            req,
            &Registry::default(),
        )
        .await
        .unwrap();
        assert_eq!(resp.text, "listo");
        assert_eq!(*calls.lock().unwrap(), 2);
    }
}
