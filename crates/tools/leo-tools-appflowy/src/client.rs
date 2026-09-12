use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::Mutex;

use crate::error::{Error, clip, env};

#[derive(Clone)]
pub struct Config {
    pub base_url: String,
    pub gotrue_url: String,
    pub email: Option<String>,
    pub password: Option<String>,
    pub access_token: Option<String>,
    pub workspace_id: Option<String>,
    pub parent_view_id: Option<String>,
}

impl Config {
    pub fn from_env() -> Option<Self> {
        let base = env("APPFLOWY_BASE_URL")?;
        let token = env("APPFLOWY_ACCESS_TOKEN");
        let email = env("APPFLOWY_EMAIL");
        let password = env("APPFLOWY_PASSWORD");
        if token.is_none() && (email.is_none() || password.is_none()) {
            return None;
        }
        let gotrue = env("APPFLOWY_GOTRUE_URL")
            .unwrap_or_else(|| format!("{}/gotrue", base.trim_end_matches('/')));
        Some(Self {
            base_url: base.trim_end_matches('/').to_string(),
            gotrue_url: gotrue.trim_end_matches('/').to_string(),
            email,
            password,
            access_token: token,
            workspace_id: env("APPFLOWY_WORKSPACE_ID"),
            parent_view_id: env("APPFLOWY_PARENT_VIEW_ID"),
        })
    }
}

struct Token {
    access: String,
    refresh: Option<String>,
    expires_at: Instant,
}

#[derive(Clone)]
pub struct Client {
    cfg: Config,
    http: reqwest::Client,
    token: std::sync::Arc<Mutex<Option<Token>>>,
}

impl Client {
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg,
            http: reqwest::Client::new(),
            token: std::sync::Arc::new(Mutex::new(None)),
        }
    }

    pub fn from_env() -> Option<Self> {
        Config::from_env().map(Self::new)
    }

    pub fn parent_view_id(&self) -> Option<&str> {
        self.cfg.parent_view_id.as_deref()
    }

    pub async fn workspace_id(&self) -> Result<String, Error> {
        if let Some(id) = &self.cfg.workspace_id {
            return Ok(id.clone());
        }
        let list: Vec<Workspace> = self.get_json("/api/workspace").await?;
        list.first()
            .and_then(|w| w.id())
            .ok_or_else(|| Error::Msg("no hay workspaces en AppFlowy".into()))
    }

    pub async fn default_parent(&self, workspace: &str) -> Result<String, Error> {
        let folder: Folder = self
            .get_json(&format!("/api/workspace/{workspace}/folder"))
            .await?;
        folder
            .children
            .iter()
            .find(|c| c.is_space)
            .or(folder.children.first())
            .map(|c| c.view_id.clone())
            .or(folder.view_id)
            .ok_or_else(|| Error::Msg("no hay un espacio padre en el workspace".into()))
    }

    pub async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, Error> {
        self.request(reqwest::Method::GET, path, None).await
    }

    pub async fn post_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, Error> {
        self.request(reqwest::Method::POST, path, Some(body)).await
    }

    pub async fn patch_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, Error> {
        self.request(reqwest::Method::PATCH, path, Some(body)).await
    }

    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<T, Error> {
        let token = self.ensure_token(false).await?;
        match self.send(method.clone(), path, body, &token).await {
            Ok(v) => Ok(v),
            Err(Error::Http { status: 401, .. }) => {
                let token = self.ensure_token(true).await?;
                self.send(method, path, body, &token).await
            }
            Err(err) => Err(err),
        }
    }

    async fn send<T: for<'de> Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
        token: &str,
    ) -> Result<T, Error> {
        let url = format!("{}{path}", self.cfg.base_url);
        let mut builder = self
            .http
            .request(method, url)
            .bearer_auth(token)
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
        unwrap_data(&text)
    }

    async fn ensure_token(&self, force: bool) -> Result<String, Error> {
        if let Some(static_token) = &self.cfg.access_token {
            return Ok(static_token.clone());
        }
        let mut guard = self.token.lock().await;
        if !force {
            if let Some(t) = guard.as_ref() {
                if Instant::now() + Duration::from_secs(60) < t.expires_at {
                    return Ok(t.access.clone());
                }
            }
        }
        if let Some(refresh) = guard.as_ref().and_then(|t| t.refresh.clone()) {
            if let Ok(minted) = self.refresh(&refresh).await {
                let access = minted.access.clone();
                *guard = Some(minted);
                return Ok(access);
            }
        }
        let minted = self.login().await?;
        let access = minted.access.clone();
        *guard = Some(minted);
        Ok(access)
    }

    async fn login(&self) -> Result<Token, Error> {
        let email = self
            .cfg
            .email
            .as_deref()
            .ok_or(Error::Missing("APPFLOWY_EMAIL"))?;
        let password = self
            .cfg
            .password
            .as_deref()
            .ok_or(Error::Missing("APPFLOWY_PASSWORD"))?;
        self.gotrue(
            serde_json::json!({
                "grant_type": "password",
                "email": email,
                "password": password,
            }),
            "password",
        )
        .await
    }

    async fn refresh(&self, refresh_token: &str) -> Result<Token, Error> {
        self.gotrue(
            serde_json::json!({
                "grant_type": "refresh_token",
                "refresh_token": refresh_token,
            }),
            "refresh_token",
        )
        .await
    }

    async fn gotrue(&self, body: serde_json::Value, grant: &str) -> Result<Token, Error> {
        let url = format!("{}/token?grant_type={grant}", self.cfg.gotrue_url);
        let response = self.http.post(url).json(&body).send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(Error::Http {
                status: status.as_u16(),
                body: clip(&text),
            });
        }
        let parsed: GoTrue = serde_json::from_str(&text)?;
        let access = parsed
            .access_token
            .filter(|s| !s.is_empty())
            .ok_or_else(|| Error::Msg("gotrue no devolvió access_token".into()))?;
        Ok(Token {
            access,
            refresh: parsed.refresh_token,
            expires_at: Instant::now() + Duration::from_secs(parsed.expires_in.unwrap_or(3600)),
        })
    }
}

#[derive(Deserialize)]
struct Workspace {
    #[serde(default)]
    workspace_id: Option<String>,
    #[serde(default)]
    id: Option<String>,
}

impl Workspace {
    fn id(&self) -> Option<String> {
        self.workspace_id.clone().or(self.id.clone())
    }
}

#[derive(Deserialize)]
struct Folder {
    #[serde(default)]
    view_id: Option<String>,
    #[serde(default)]
    children: Vec<FolderChild>,
}

#[derive(Deserialize)]
struct FolderChild {
    view_id: String,
    #[serde(default)]
    is_space: bool,
}

#[derive(Deserialize)]
struct GoTrue {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

fn unwrap_data<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, Error> {
    if text.trim().is_empty() {
        return serde_json::from_value(serde_json::json!({})).map_err(Error::from);
    }
    let v: serde_json::Value = serde_json::from_str(text)?;
    let payload = v.get("data").cloned().unwrap_or(v);
    Ok(serde_json::from_value(payload)?)
}
