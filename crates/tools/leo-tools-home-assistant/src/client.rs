use crate::error::{Error, clip, env};

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
    token: String,
}

impl Client {
    pub fn from_env() -> Option<Self> {
        let base = env("HOME_ASSISTANT_URL").or_else(|| env("HASS_URL"))?;
        let token = env("HOME_ASSISTANT_TOKEN").or_else(|| env("HASS_TOKEN"))?;
        Some(Self {
            http: reqwest::Client::new(),
            base: base.trim_end_matches('/').to_string(),
            token,
        })
    }

    pub async fn get(&self, path: &str) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::GET, path, None).await
    }

    pub async fn post(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::POST, path, Some(body)).await
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, Error> {
        let url = format!("{}{path}", self.base);
        let mut builder = self
            .http
            .request(method, url)
            .bearer_auth(&self.token)
            .header("Content-Type", "application/json");
        if let Some(body) = body {
            builder = builder.json(body);
        }
        let response = builder.send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(Error::Http {
                status: status.as_u16(),
                body: clip(&text),
            });
        }
        if text.trim().is_empty() {
            return Ok(serde_json::json!({ "ok": true }));
        }
        Ok(serde_json::from_str(&text)?)
    }
}
