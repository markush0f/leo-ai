use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use ira_store::{McpServerConfig, McpTransport};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::error::Error;
use crate::http::HttpSession;
use crate::rpc::{self, RemoteTool};
use crate::stdio::StdioSession;
use crate::vars::resolve_config;

const HTTP_TIMEOUT: Duration = Duration::from_secs(5);
const STDIO_TIMEOUT: Duration = Duration::from_secs(30);
const CALL_TIMEOUT: Duration = Duration::from_secs(60);
const FAIL_TTL: Duration = Duration::from_secs(30);

#[derive(Clone)]
enum Session {
    Stdio(Arc<StdioSession>),
    Http(HttpSession),
}

impl Session {
    async fn alive(&self) -> bool {
        match self {
            Self::Stdio(session) => session.alive().await,
            Self::Http(_) => true,
        }
    }

    async fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, Error> {
        match self {
            Self::Stdio(session) => session.request(method, params, timeout).await,
            Self::Http(session) => session.request(method, params, timeout).await,
        }
    }
}

struct Slot {
    fingerprint: String,
    session: Session,
}

pub struct McpManager {
    slots: Mutex<HashMap<String, Slot>>,
    fails: Mutex<HashMap<String, (Instant, String)>>,
}

static SHARED: OnceLock<McpManager> = OnceLock::new();

pub fn shared() -> &'static McpManager {
    SHARED.get_or_init(McpManager::new)
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
            fails: Mutex::new(HashMap::new()),
        }
    }

    pub async fn connect(&self, config: &McpServerConfig) -> Result<(), Error> {
        config
            .validate()
            .map_err(Error::msg)?;
        let resolved = resolve_config(config)?;
        let fingerprint = fingerprint(&resolved);
        let reuse = {
            let slots = self.slots.lock().await;
            if let Some(slot) = slots.get(&config.id) {
                slot.fingerprint == fingerprint && slot.session.alive().await
            } else {
                false
            }
        };
        if reuse {
            return Ok(());
        }
        if let Some((at, message)) = self.fails.lock().await.get(&config.id)
            && at.elapsed() < FAIL_TTL
        {
            return Err(Error::msg(message.clone()));
        }
        let timeout = match resolved.transport {
            McpTransport::Stdio => STDIO_TIMEOUT,
            McpTransport::StreamableHttp => HTTP_TIMEOUT,
        };
        match open(&resolved, timeout).await {
            Ok(session) => {
                self.fails.lock().await.remove(&config.id);
                self.slots.lock().await.insert(
                    config.id.clone(),
                    Slot {
                        fingerprint,
                        session,
                    },
                );
                Ok(())
            }
            Err(err) => {
                self.fails
                    .lock()
                    .await
                    .insert(config.id.clone(), (Instant::now(), err.to_string()));
                Err(err)
            }
        }
    }

    pub async fn disconnect(&self, id: &str) {
        self.slots.lock().await.remove(id);
        self.fails.lock().await.remove(id);
    }

    pub async fn list_tools(&self, id: &str) -> Result<Vec<RemoteTool>, Error> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut params = json!({});
            if let Some(cursor) = &cursor {
                params["cursor"] = json!(cursor);
            }
            let result = self.rpc(id, "tools/list", params, HTTP_TIMEOUT).await?;
            tools.extend(rpc::parse_tools_list(&result)?);
            cursor = result
                .get("nextCursor")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        Ok(tools)
    }

    pub async fn call_tool(&self, id: &str, name: &str, arguments: Value) -> Result<Value, Error> {
        if name.trim().is_empty() {
            return Err(Error::msg("falta el nombre de la herramienta"));
        }
        let args = match arguments {
            Value::Null => json!({}),
            Value::Object(_) => arguments,
            Value::String(text) => serde_json::from_str(&text).unwrap_or(json!({ "_": text })),
            other => json!({ "value": other }),
        };
        let result = self
            .rpc(
                id,
                "tools/call",
                json!({ "name": name, "arguments": args }),
                CALL_TIMEOUT,
            )
            .await?;
        rpc::parse_call_result(&result)
    }

    pub async fn list_resources(&self, id: &str) -> Result<Value, Error> {
        self.rpc(id, "resources/list", json!({}), HTTP_TIMEOUT).await
    }

    pub async fn list_prompts(&self, id: &str) -> Result<Value, Error> {
        self.rpc(id, "prompts/list", json!({}), HTTP_TIMEOUT).await
    }

    async fn rpc(
        &self,
        id: &str,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, Error> {
        let session = self
            .slots
            .lock()
            .await
            .get(id)
            .map(|slot| slot.session.clone())
            .ok_or_else(|| Error::msg(format!("mcp {id} no conectado")))?;
        session.request(method, params, timeout).await
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

async fn open(config: &McpServerConfig, timeout: Duration) -> Result<Session, Error> {
    match config.transport {
        McpTransport::Stdio => {
            let command = config
                .command
                .as_deref()
                .ok_or_else(|| Error::msg("stdio sin command"))?;
            let session = StdioSession::spawn(command, &config.args, &config.env, timeout).await?;
            Ok(Session::Stdio(Arc::new(session)))
        }
        McpTransport::StreamableHttp => {
            let url = config
                .url
                .as_deref()
                .ok_or_else(|| Error::msg("http sin url"))?;
            let session = HttpSession::new(url, config.headers.clone());
            session.ensure(timeout).await?;
            Ok(Session::Http(session))
        }
    }
}

fn fingerprint(config: &McpServerConfig) -> String {
    let mut env: Vec<_> = config.env.iter().collect();
    env.sort_by(|left, right| left.0.cmp(right.0));
    let mut headers: Vec<_> = config.headers.iter().collect();
    headers.sort_by(|left, right| left.0.cmp(right.0));
    format!(
        "{}|{}|{:?}|{env:?}|{headers:?}|{:?}",
        config.transport.as_str(),
        config.command.as_deref().unwrap_or(""),
        config.args,
        config.url
    )
}
