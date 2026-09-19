use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::provider::{Provider, ProviderFuture};
use crate::{ChatRequest, ChatResponse, LlmError, ProviderId, Role, ToolCall};

use super::{AuthManager, CodexConfig, MemoryTokenStore, OAuthCredentials, TokenStore};

// Codex backend filters its catalog by Codex CLI protocol version, not Leo's package version.
const CODEX_MODELS_CLIENT_VERSION: &str = "0.155.0";

#[derive(Clone)]
pub struct Codex {
    http: reqwest::Client,
    auth: AuthManager,
    config: CodexConfig,
}

impl Codex {
    pub fn new(store: Arc<dyn TokenStore>) -> Self {
        Self::with_config(store, CodexConfig::default())
    }

    pub fn with_config(store: Arc<dyn TokenStore>, config: CodexConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            auth: AuthManager::new(store),
            config,
        }
    }

    pub fn from_credentials(credentials: OAuthCredentials) -> Self {
        Self::new(Arc::new(MemoryTokenStore::new(credentials)))
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.config.default_model = model.into();
        self
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.endpoint = endpoint.into();
        self
    }

    pub async fn complete(&self, request: ChatRequest) -> Result<ChatResponse, LlmError> {
        let mut credentials = self.auth.authorization().await?;
        let model = request
            .model
            .as_deref()
            .unwrap_or(&self.config.default_model);
        let body = request_body(model, &request);
        let mut response = self.send(&credentials, &body).await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            credentials = self.auth.refresh_after(&credentials.access_token).await?;
            response = self.send(&credentials, &body).await?;
        }
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(LlmError::Http {
                status: status.as_u16(),
                body: crate::extract_error_message(&body).unwrap_or(body),
            });
        }
        parse_response(model, &body)
    }

    pub async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let mut credentials = self.auth.authorization().await?;
        let mut response = self.send_models(&credentials).await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            credentials = self.auth.refresh_after(&credentials.access_token).await?;
            response = self.send_models(&credentials).await?;
        }
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(LlmError::Http {
                status: status.as_u16(),
                body: crate::extract_error_message(&body).unwrap_or(body),
            });
        }
        parse_models(&body)
    }

    async fn send(
        &self,
        credentials: &OAuthCredentials,
        body: &Value,
    ) -> Result<reqwest::Response, LlmError> {
        let mut builder = self
            .http
            .post(&self.config.endpoint)
            .bearer_auth(&credentials.access_token)
            .header("originator", &self.config.originator)
            .header("User-Agent", "leo-ai")
            .json(body);
        if let Some(account_id) = &credentials.account_id {
            builder = builder.header("ChatGPT-Account-Id", account_id);
        }
        Ok(builder.send().await?)
    }

    async fn send_models(
        &self,
        credentials: &OAuthCredentials,
    ) -> Result<reqwest::Response, LlmError> {
        let base = self
            .config
            .endpoint
            .trim_end_matches('/')
            .strip_suffix("/responses")
            .unwrap_or(self.config.endpoint.trim_end_matches('/'));
        let client_version = std::env::var("CODEX_CLIENT_VERSION")
            .ok()
            .filter(|version| !version.trim().is_empty())
            .unwrap_or_else(|| CODEX_MODELS_CLIENT_VERSION.into());
        let mut builder = self
            .http
            .get(format!("{base}/models"))
            .query(&[("client_version", client_version.as_str())])
            .timeout(Duration::from_secs(5))
            .bearer_auth(&credentials.access_token)
            .header("originator", &self.config.originator)
            .header("version", &client_version)
            .header("User-Agent", format!("leo-ai/{client_version}"));
        if let Some(account_id) = &credentials.account_id {
            builder = builder.header("ChatGPT-Account-Id", account_id);
        }
        Ok(builder.send().await?)
    }
}

impl Provider for Codex {
    fn complete(&self, request: ChatRequest) -> ProviderFuture<'_> {
        Box::pin(async move { Codex::complete(self, request).await })
    }
}

#[derive(Deserialize)]
struct ModelsResponse {
    models: Vec<CodexModel>,
}

#[derive(Deserialize)]
struct CodexModel {
    slug: String,
    #[serde(default)]
    visibility: Option<String>,
    #[serde(default)]
    priority: i64,
}

fn parse_models(body: &str) -> Result<Vec<String>, LlmError> {
    let mut models = serde_json::from_str::<ModelsResponse>(body)?.models;
    models.retain(|model| model.visibility.as_deref() == Some("list"));
    models.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.slug.cmp(&b.slug))
    });
    let mut names: Vec<String> = models.into_iter().map(|model| model.slug).collect();
    names.dedup();
    Ok(names)
}

fn request_body(model: &str, request: &ChatRequest) -> Value {
    let mut instructions = Vec::new();
    let mut input = Vec::new();
    for message in &request.messages {
        match message.role {
            Role::System => instructions.push(message.content.clone()),
            Role::User | Role::Assistant => {
                input.extend(message.provider_items.iter().cloned());
                if !message.content.is_empty() {
                    input.push(json!({
                        "role": if message.role == Role::User { "user" } else { "assistant" },
                        "content": message.content,
                    }));
                }
                for call in &message.tool_calls {
                    input.push(json!({
                        "type": "function_call",
                        "call_id": call.id,
                        "name": call.name,
                        "arguments": call.arguments,
                    }));
                }
            }
            Role::Tool => input.push(json!({
                "type": "function_call_output",
                "call_id": message.tool_call_id,
                "output": message.content,
            })),
        }
    }
    let tools: Vec<_> = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
                "strict": false,
            })
        })
        .collect();
    let mut body = json!({
        "model": model,
        "input": input,
        "tools": tools,
        "store": false,
        "stream": true,
    });
    if !instructions.is_empty() {
        body["instructions"] = instructions.join("\n").into();
    }
    if let Some(effort) = &request.reasoning_effort {
        body["reasoning"] = json!({ "effort": effort, "summary": "auto" });
        body["include"] = json!(["reasoning.encrypted_content"]);
    }
    body
}

fn parse_response(fallback_model: &str, body: &str) -> Result<ChatResponse, LlmError> {
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut provider_items = Vec::new();
    let mut model = fallback_model.to_string();
    if body.trim_start().starts_with('{') {
        let value: Value = serde_json::from_str(body)?;
        parse_response_value(
            &value,
            &mut text,
            &mut tool_calls,
            &mut provider_items,
            &mut model,
        );
    } else {
        for line in body.lines() {
            let Some(data) = line.trim().strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let event: Value = serde_json::from_str(data)?;
            match event.get("type").and_then(Value::as_str) {
                Some("response.output_text.delta") => {
                    text.push_str(
                        event
                            .get("delta")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    );
                }
                Some("response.output_item.done") => {
                    if let Some(item) = event.get("item") {
                        if let Some(call) = parse_tool_call(item) {
                            tool_calls.push(call);
                        } else if item.get("type").and_then(Value::as_str) == Some("reasoning") {
                            provider_items.push(item.clone());
                        }
                    }
                }
                Some("response.completed") => {
                    if let Some(response) = event.get("response") {
                        if let Some(value) = response.get("model").and_then(Value::as_str) {
                            model = value.to_string();
                        }
                        if text.is_empty() {
                            parse_response_value(
                                response,
                                &mut text,
                                &mut tool_calls,
                                &mut provider_items,
                                &mut model,
                            );
                        }
                    }
                }
                Some("error" | "response.failed") => {
                    return Err(LlmError::Authentication(
                        event
                            .pointer("/error/message")
                            .or_else(|| event.pointer("/response/error/message"))
                            .and_then(Value::as_str)
                            .unwrap_or("Codex devolvió un error")
                            .to_string(),
                    ));
                }
                _ => {}
            }
        }
    }
    if text.is_empty() && tool_calls.is_empty() {
        return Err(LlmError::Empty("codex"));
    }
    Ok(ChatResponse {
        provider: ProviderId::Codex,
        model,
        text,
        tool_calls,
        provider_items,
    })
}

fn parse_response_value(
    response: &Value,
    text: &mut String,
    tool_calls: &mut Vec<ToolCall>,
    provider_items: &mut Vec<Value>,
    model: &mut String,
) {
    if let Some(value) = response.get("model").and_then(Value::as_str) {
        *model = value.to_string();
    }
    let Some(output) = response.get("output").and_then(Value::as_array) else {
        return;
    };
    for item in output {
        if let Some(call) = parse_tool_call(item) {
            tool_calls.push(call);
        } else if item.get("type").and_then(Value::as_str) == Some("reasoning") {
            provider_items.push(item.clone());
        }
        if let Some(content) = item.get("content").and_then(Value::as_array) {
            for part in content {
                if part.get("type").and_then(Value::as_str) == Some("output_text") {
                    text.push_str(part.get("text").and_then(Value::as_str).unwrap_or_default());
                }
            }
        }
    }
}

fn parse_tool_call(item: &Value) -> Option<ToolCall> {
    (item.get("type")?.as_str()? == "function_call").then(|| ToolCall {
        id: item
            .get("call_id")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        name: item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        arguments: item
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or("{}")
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_sse() {
        let body = concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hola\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"model\":\"gpt-5.4\"}}\n\n"
        );
        let response = parse_response("fallback", body).unwrap();
        assert_eq!(response.text, "Hola");
        assert_eq!(response.model, "gpt-5.4");
    }

    #[test]
    fn builds_responses_payload() {
        let mut request = ChatRequest::user("hola").with_system("sé breve");
        request.max_tokens = Some(512);
        let body = request_body("gpt-5.4", &request);
        assert_eq!(body["instructions"], "sé breve");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["store"], false);
        assert!(body.get("max_output_tokens").is_none());
    }

    #[test]
    fn parses_visible_models_by_priority() {
        let names = parse_models(
            r#"{"models":[
                {"slug":"gpt-b","visibility":"list","priority":2},
                {"slug":"hidden","visibility":"hide","priority":0},
                {"slug":"unspecified","priority":0},
                {"slug":"gpt-a","visibility":"list","priority":1}
            ]}"#,
        )
        .unwrap();
        assert_eq!(names, ["gpt-a", "gpt-b"]);
    }

    #[test]
    fn preserves_encrypted_reasoning_for_tool_follow_up() {
        let body = concat!(
            "data: {\"type\":\"response.output_item.done\",\"item\":",
            "{\"type\":\"reasoning\",\"encrypted_content\":\"opaque\",\"summary\":[]}}\n\n",
            "data: {\"type\":\"response.output_item.done\",\"item\":",
            "{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"weather\",\"arguments\":\"{}\"}}\n\n"
        );
        let response = parse_response("gpt-5.4", body).unwrap();
        let message = crate::ChatMessage::assistant_tools_with_provider_items(
            response.text,
            response.tool_calls,
            response.provider_items,
        );
        let follow_up = ChatRequest {
            messages: vec![message],
            ..ChatRequest::default()
        };
        let request = request_body("gpt-5.4", &follow_up);
        assert_eq!(request["input"][0]["encrypted_content"], "opaque");
        assert_eq!(request["input"][1]["call_id"], "call_1");
    }
}
