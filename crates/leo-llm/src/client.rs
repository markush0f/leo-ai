use crate::error::LlmError;
use crate::types::{ChatRequest, ChatResponse, ProviderId};
use crate::{claude, ollama, openai_compat};

#[derive(Clone)]
pub struct Client {
    provider: ProviderId,
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    default_model: String,
}

impl Client {
    pub fn grok(api_key: impl Into<String>) -> Self {
        Self::new(ProviderId::Grok, Some(api_key.into()), None)
    }

    pub fn gpt(api_key: impl Into<String>) -> Self {
        Self::new(ProviderId::Gpt, Some(api_key.into()), None)
    }

    pub fn ollama(host: impl Into<String>) -> Self {
        Self::new(
            ProviderId::Ollama,
            None,
            Some(ollama::origin(&host.into())),
        )
    }

    pub fn claude(api_key: impl Into<String>) -> Self {
        Self::new(ProviderId::Claude, Some(api_key.into()), None)
    }

    pub fn from_env(provider: ProviderId) -> Result<Self, LlmError> {
        Self::connect(provider, None, None)
    }

    /// Construye el cliente con clave/url guardadas; si faltan, usa el entorno.
    pub fn connect(
        provider: ProviderId,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> Result<Self, LlmError> {
        let api_key = nonempty(api_key).or_else(|| match provider.env_key() {
            Some(var) => nonempty(std::env::var(var).ok()),
            None => None,
        });
        if api_key.is_none() {
            if let Some(var) = provider.env_key() {
                return Err(LlmError::MissingKey(var));
            }
        }

        let base_url = nonempty(base_url).or_else(|| {
            if provider == ProviderId::Ollama {
                nonempty(std::env::var("OLLAMA_HOST").ok())
            } else {
                None
            }
        });
        let base_url = base_url.map(|h| {
            if provider == ProviderId::Ollama {
                ollama::origin(&h)
            } else {
                h.trim().trim_end_matches('/').to_string()
            }
        });
        Ok(Self::new(provider, api_key, base_url))
    }

    fn new(provider: ProviderId, api_key: Option<String>, base_url: Option<String>) -> Self {
        Self {
            provider,
            http: reqwest::Client::new(),
            base_url: base_url.unwrap_or_else(|| provider.default_base_url().to_string()),
            api_key,
            default_model: provider.default_model().to_string(),
        }
    }

    pub fn provider(&self) -> ProviderId {
        self.provider
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_string();
        self
    }

    pub async fn chat_text(&self, prompt: impl Into<String>) -> Result<ChatResponse, LlmError> {
        self.chat(ChatRequest::user(prompt)).await
    }

    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LlmError> {
        let model = req
            .model
            .as_deref()
            .unwrap_or(self.default_model.as_str());
        match self.provider {
            ProviderId::Grok | ProviderId::Gpt => self.chat_openai(model, &req).await,
            ProviderId::Ollama => self.chat_ollama(model, &req).await,
            ProviderId::Claude => self.chat_claude(model, &req).await,
        }
    }

    async fn chat_openai(
        &self,
        model: &str,
        req: &ChatRequest,
    ) -> Result<ChatResponse, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut builder = self.http.post(&url).json(&openai_compat::build_body(model, req)?);
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }
        let response = builder.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(http_error(status.as_u16(), body));
        }
        openai_compat::parse_response(self.provider, model, &body)
    }

    async fn chat_ollama(
        &self,
        model: &str,
        req: &ChatRequest,
    ) -> Result<ChatResponse, LlmError> {
        let url = format!("{}/api/chat", ollama::origin(&self.base_url));
        let response = self
            .http
            .post(&url)
            .json(&ollama::build_body(model, req)?)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(http_error(status.as_u16(), body));
        }
        ollama::parse_response(model, &body)
    }

    /// Modelos instalados (`GET /api/tags`).
    pub async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        if self.provider != ProviderId::Ollama {
            return Ok(Vec::new());
        }
        let url = format!("{}/api/tags", ollama::origin(&self.base_url));
        let response = self.http.get(&url).send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(http_error(status.as_u16(), body));
        }
        ollama::parse_tags(&body)
    }

    async fn chat_claude(
        &self,
        model: &str,
        req: &ChatRequest,
    ) -> Result<ChatResponse, LlmError> {
        let url = format!("{}/messages", self.base_url);
        let mut builder = self
            .http
            .post(&url)
            .header("anthropic-version", "2023-06-01")
            .json(&claude::build_body(model, req)?);
        if let Some(key) = &self.api_key {
            builder = builder.header("x-api-key", key);
        }
        let response = builder.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(http_error(status.as_u16(), body));
        }
        claude::parse_response(model, &body)
    }
}

fn http_error(status: u16, body: String) -> LlmError {
    LlmError::Http {
        status,
        body: extract_error_message(&body).unwrap_or(body),
    }
}

pub fn extract_error_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let err = v.get("error")?;
    err.as_str()
        .map(str::to_string)
        .or_else(|| err.get("message")?.as_str().map(str::to_string))
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    })
}
