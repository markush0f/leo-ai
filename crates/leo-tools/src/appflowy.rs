use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::Mutex;

use crate::markdown;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("falta {0} en el entorno")]
    Missing(&'static str),
    #[error("appflowy http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("appflowy: {0}")]
    Msg(String),
    #[error("red: {0}")]
    Network(#[from] reqwest::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

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

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    })
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

    pub async fn create_page(
        &self,
        title: &str,
        markdown: &str,
        parent_view_id: Option<&str>,
    ) -> Result<Created, Error> {
        let workspace = self.workspace_id().await?;
        let parent = match parent_view_id.filter(|s| !s.is_empty()) {
            Some(id) => id.to_string(),
            None => match self.cfg.parent_view_id.clone() {
                Some(id) => id,
                None => self.default_parent(&workspace).await?,
            },
        };
        let created: Page = self
            .post_json(
                &format!("/api/workspace/{workspace}/page-view"),
                &serde_json::json!({
                    "parent_view_id": parent,
                    "layout": 0,
                    "name": title,
                }),
            )
            .await?;
        if !markdown.trim().is_empty() {
            let blocks = markdown::to_blocks(markdown);
            self.post_json::<serde_json::Value>(
                &format!(
                    "/api/workspace/{workspace}/page-view/{}/append-block",
                    created.view_id
                ),
                &serde_json::json!({ "blocks": blocks }),
            )
            .await?;
        }
        Ok(Created {
            view_id: created.view_id,
            title: title.to_string(),
            workspace_id: workspace,
        })
    }

    async fn workspace_id(&self) -> Result<String, Error> {
        if let Some(id) = &self.cfg.workspace_id {
            return Ok(id.clone());
        }
        let list: Vec<Workspace> = self.get_json("/api/workspace").await?;
        list.first()
            .and_then(|w| w.id())
            .ok_or_else(|| Error::Msg("no hay workspaces en AppFlowy".into()))
    }

    async fn default_parent(&self, workspace: &str) -> Result<String, Error> {
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

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, Error> {
        self.request(reqwest::Method::GET, path, None).await
    }

    async fn post_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, Error> {
        self.request(reqwest::Method::POST, path, Some(body)).await
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

#[derive(Debug)]
pub struct Created {
    pub view_id: String,
    pub title: String,
    pub workspace_id: String,
}

#[derive(Deserialize)]
struct Page {
    view_id: String,
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
    let v: serde_json::Value = serde_json::from_str(text)?;
    let payload = v.get("data").cloned().unwrap_or(v);
    Ok(serde_json::from_value(payload)?)
}

fn clip(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > 400 {
        format!("{}…", t.chars().take(400).collect::<String>())
    } else {
        t.to_string()
    }
}

pub fn spec() -> leo_llm::ToolSpec {
    leo_llm::ToolSpec {
        name: "appflowy_write".into(),
        description: "Crea una página nueva en AppFlowy con el título y el cuerpo en markdown. \
Usa esto cuando el usuario pida guardar, anotar o escribir algo en AppFlowy."
            .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Título de la página"
                },
                "markdown": {
                    "type": "string",
                    "description": "Contenido en markdown (encabezados, listas, código, citas)"
                },
                "parent_view_id": {
                    "type": "string",
                    "description": "Opcional: view_id del espacio o página padre"
                }
            },
            "required": ["title", "markdown"]
        }),
    }
}

pub async fn call(client: &Client, args: serde_json::Value) -> String {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let markdown = args
        .get("markdown")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let parent = args.get("parent_view_id").and_then(|v| v.as_str());
    if title.is_empty() {
        return "error: falta title".into();
    }
    match client.create_page(title, markdown, parent).await {
        Ok(page) => serde_json::json!({
            "ok": true,
            "view_id": page.view_id,
            "title": page.title,
            "workspace_id": page.workspace_id,
        })
        .to_string(),
        Err(err) => serde_json::json!({
            "ok": false,
            "error": err.to_string(),
        })
        .to_string(),
    }
}
