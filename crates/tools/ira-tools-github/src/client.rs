use crate::error::{Error, clip, env};

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    token: String,
    base: String,
}

impl Client {
    pub fn from_env() -> Option<Self> {
        let token = env("GITHUB_TOKEN").or_else(|| env("GH_TOKEN"))?;
        let base = env("GITHUB_API_URL").unwrap_or_else(|| "https://api.github.com".into());
        Some(Self {
            http: reqwest::Client::new(),
            token,
            base: base.trim_end_matches('/').to_string(),
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
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "ira-ai");
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
            return Ok(serde_json::json!({}));
        }
        Ok(serde_json::from_str(&text)?)
    }
}

pub fn repo_path(owner: &str, repo: &str) -> Result<String, Error> {
    let owner = owner.trim();
    let repo = repo.trim().trim_end_matches(".git");
    if owner.is_empty() || repo.is_empty() {
        return Err(Error::msg("falta owner o repo"));
    }
    if owner.contains('/') {
        let (o, r) = owner.split_once('/').unwrap();
        return Ok(format!("/repos/{o}/{r}"));
    }
    Ok(format!("/repos/{owner}/{repo}"))
}
