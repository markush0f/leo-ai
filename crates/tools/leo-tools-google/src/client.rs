use crate::error::{Error, clip, env};

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    token: String,
}

impl Client {
    pub fn from_env() -> Option<Self> {
        let token = env("GOOGLE_ACCESS_TOKEN").or_else(|| env("GOOGLE_API_KEY"))?;
        Some(Self {
            http: reqwest::Client::new(),
            token,
        })
    }

    pub async fn get(&self, url: &str) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::GET, url, None).await
    }

    pub async fn post(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::POST, url, Some(body)).await
    }

    pub async fn patch(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::PATCH, url, Some(body)).await
    }

    pub async fn delete(&self, url: &str) -> Result<serde_json::Value, Error> {
        self.request(reqwest::Method::DELETE, url, None).await
    }

    async fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, Error> {
        let mut builder = self.http.request(method, url);
        if env("GOOGLE_ACCESS_TOKEN").is_some() {
            builder = builder.bearer_auth(&self.token);
        } else {
            builder = builder.query(&[("key", self.token.as_str())]);
        }
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

pub fn calendar_url(calendar_id: &str, rest: &str) -> String {
    let id = if calendar_id.trim().is_empty() {
        "primary"
    } else {
        calendar_id.trim()
    };
    let enc = urlencoding(id);
    format!("https://www.googleapis.com/calendar/v3/calendars/{enc}{rest}")
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
