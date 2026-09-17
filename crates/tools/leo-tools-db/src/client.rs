use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::error::{Error, clip, env};

const PROTOCOL: &str = "2025-03-26";

/// MCP Toolbox HTTP client. Talks JSON-RPC to `/mcp` (optional toolset suffix).
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    mcp_url: String,
    token: Option<String>,
    next_id: AtomicU64,
    session: Mutex<Session>,
}

struct Session {
    id: Option<String>,
    protocol: String,
    ready: bool,
}

/// One tool advertised by Toolbox (`tools/list`).
#[derive(Debug, Clone)]
pub struct RemoteTool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl Client {
    pub fn new(
        base_url: impl Into<String>,
        toolset: Option<String>,
        token: Option<String>,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            inner: Arc::new(Inner {
                http,
                mcp_url: mcp_endpoint(&base_url.into(), toolset.as_deref()),
                token,
                next_id: AtomicU64::new(1),
                session: Mutex::new(Session {
                    id: None,
                    protocol: PROTOCOL.to_string(),
                    ready: false,
                }),
            }),
        }
    }

    /// `MCP_TOOLBOX_URL` or `TOOLBOX_URL`. Optional toolset and bearer token.
    pub fn from_env() -> Option<Self> {
        let base = env("MCP_TOOLBOX_URL").or_else(|| env("TOOLBOX_URL"))?;
        let toolset = env("MCP_TOOLBOX_TOOLSET").or_else(|| env("TOOLBOX_TOOLSET"));
        let token = env("MCP_TOOLBOX_TOKEN")
            .or_else(|| env("TOOLBOX_TOKEN"))
            .or_else(|| env("MCP_TOOLBOX_API_KEY"));
        Some(Self::new(base, toolset, token))
    }

    pub fn mcp_url(&self) -> &str {
        &self.inner.mcp_url
    }

    pub async fn list_tools(&self) -> Result<Vec<RemoteTool>, Error> {
        self.ensure_session().await?;
        let result = self
            .rpc("tools/list", json!({}), false)
            .await?
            .ok_or_else(|| Error::msg("tools/list sin resultado"))?;
        parse_tools_list(&result)
    }

    pub async fn invoke(&self, name: &str, arguments: Value) -> Result<Value, Error> {
        if name.trim().is_empty() {
            return Err(Error::msg("falta el nombre de la herramienta"));
        }
        self.ensure_session().await?;
        let args = match arguments {
            Value::Null => json!({}),
            Value::Object(_) => arguments,
            Value::String(s) => serde_json::from_str(&s).unwrap_or(json!({ "_": s })),
            other => json!({ "value": other }),
        };
        let result = self
            .rpc(
                "tools/call",
                json!({ "name": name, "arguments": args }),
                false,
            )
            .await?
            .ok_or_else(|| Error::msg("tools/call sin resultado"))?;
        parse_call_result(&result)
    }

    async fn ensure_session(&self) -> Result<(), Error> {
        {
            let g = self.inner.session.lock().await;
            if g.ready {
                return Ok(());
            }
        }
        self.handshake().await
    }

    async fn handshake(&self) -> Result<(), Error> {
        let params = json!({
            "protocolVersion": PROTOCOL,
            "capabilities": {},
            "clientInfo": { "name": "leo", "version": env!("CARGO_PKG_VERSION") },
        });
        let (headers, result) = self
            .rpc_raw("initialize", params, false, None, PROTOCOL)
            .await?;
        let result = result.ok_or_else(|| Error::msg("initialize sin resultado"))?;
        let protocol = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or(PROTOCOL)
            .to_string();
        let session_id = headers
            .get("mcp-session-id")
            .or_else(|| headers.get("Mcp-Session-Id"))
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        {
            let mut g = self.inner.session.lock().await;
            g.id = session_id.clone();
            g.protocol = protocol.clone();
            g.ready = false;
        }
        let _ = self
            .rpc_raw(
                "notifications/initialized",
                json!({}),
                true,
                session_id.as_deref(),
                &protocol,
            )
            .await?;
        let mut g = self.inner.session.lock().await;
        g.ready = true;
        Ok(())
    }

    async fn rpc(
        &self,
        method: &str,
        params: Value,
        notification: bool,
    ) -> Result<Option<Value>, Error> {
        let (id, protocol) = {
            let g = self.inner.session.lock().await;
            (g.id.clone(), g.protocol.clone())
        };
        let (_, result) = self
            .rpc_raw(method, params, notification, id.as_deref(), &protocol)
            .await?;
        Ok(result)
    }

    async fn rpc_raw(
        &self,
        method: &str,
        params: Value,
        notification: bool,
        session_id: Option<&str>,
        protocol: &str,
    ) -> Result<(reqwest::header::HeaderMap, Option<Value>), Error> {
        let mut body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        if !notification {
            let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
            body["id"] = json!(id);
        }
        let mut builder = self
            .inner
            .http
            .post(&self.inner.mcp_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", protocol)
            .json(&body);
        if let Some(token) = &self.inner.token {
            builder = builder.bearer_auth(token);
        }
        if let Some(sid) = session_id.filter(|s| !s.is_empty()) {
            builder = builder.header("Mcp-Session-Id", sid);
        }
        let response = builder.send().await?;
        let status = response.status();
        let headers = response.headers().clone();
        let content_type = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let text = response.text().await?;
        if notification
            && (status.as_u16() == 202 || status.as_u16() == 204 || text.trim().is_empty())
        {
            return Ok((headers, None));
        }
        if !status.is_success() && status.as_u16() != 202 {
            if let Ok(rpc) = decode_rpc(&content_type, &text)
                && let Some(err) = rpc_error(&rpc)
            {
                return Err(err);
            }
            return Err(Error::Http {
                status: status.as_u16(),
                body: clip(&text),
            });
        }
        if text.trim().is_empty() {
            return Ok((headers, None));
        }
        let rpc = decode_rpc(&content_type, &text)?;
        if let Some(err) = rpc_error(&rpc) {
            return Err(err);
        }
        Ok((headers, rpc.get("result").cloned()))
    }
}

pub(crate) fn mcp_endpoint(base: &str, toolset: Option<&str>) -> String {
    let base = base.trim().trim_end_matches('/');
    let toolset = toolset.map(str::trim).filter(|s| !s.is_empty());
    if let Some(rest) = base.strip_suffix("/mcp") {
        return match toolset {
            Some(ts) => format!("{rest}/mcp/{ts}"),
            None => format!("{rest}/mcp"),
        };
    }
    if base.contains("/mcp/") {
        return base.to_string();
    }
    match toolset {
        Some(ts) => format!("{base}/mcp/{ts}"),
        None => format!("{base}/mcp"),
    }
}

fn decode_rpc(content_type: &str, body: &str) -> Result<Value, Error> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    let payload = if content_type.contains("event-stream")
        || trimmed.starts_with("event:")
        || trimmed.starts_with("data:")
    {
        sse_data(trimmed).ok_or_else(|| Error::msg("respuesta SSE vacía"))?
    } else {
        trimmed.to_string()
    };
    Ok(serde_json::from_str(&payload)?)
}

fn sse_data(body: &str) -> Option<String> {
    let mut last = None;
    for line in body.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if !data.is_empty() && data != "[DONE]" {
                last = Some(data.to_string());
            }
        }
    }
    last
}

fn rpc_error(rpc: &Value) -> Option<Error> {
    let err = rpc.get("error")?;
    Some(Error::Rpc {
        code: err.get("code").and_then(Value::as_i64).unwrap_or(0),
        message: err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("error rpc")
            .to_string(),
    })
}

fn parse_tools_list(result: &Value) -> Result<Vec<RemoteTool>, Error> {
    let tools = result
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::msg("tools/list sin array tools"))?;
    let mut out = Vec::with_capacity(tools.len());
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::msg("herramienta sin nombre"))?;
        let description = tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let parameters = tool
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
        out.push(RemoteTool {
            name: name.to_string(),
            description,
            parameters,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn parse_call_result(result: &Value) -> Result<Value, Error> {
    if result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let text = content_text(result);
        return Err(Error::msg(if text.is_empty() {
            "la herramienta devolvió un error".into()
        } else {
            text
        }));
    }
    if let Some(structured) = result.get("structuredContent")
        && !structured.is_null()
    {
        return Ok(structured.clone());
    }
    let text = content_text(result);
    if text.is_empty() {
        return Ok(result.clone());
    }
    Ok(serde_json::from_str(&text).unwrap_or_else(|_| json!(text)))
}

fn content_text(result: &Value) -> String {
    result
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            if item.get("type").and_then(Value::as_str).unwrap_or("text") == "text" {
                item.get("text").and_then(Value::as_str).map(str::to_string)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_endpoint_appends_path() {
        assert_eq!(
            mcp_endpoint("http://127.0.0.1:5000", None),
            "http://127.0.0.1:5000/mcp"
        );
        assert_eq!(
            mcp_endpoint("http://127.0.0.1:5000/", Some("data")),
            "http://127.0.0.1:5000/mcp/data"
        );
        assert_eq!(
            mcp_endpoint("http://127.0.0.1:5000/mcp", None),
            "http://127.0.0.1:5000/mcp"
        );
        assert_eq!(
            mcp_endpoint("http://127.0.0.1:5000/mcp", Some("admin")),
            "http://127.0.0.1:5000/mcp/admin"
        );
        assert_eq!(
            mcp_endpoint("http://127.0.0.1:5000/mcp/custom", Some("ignored")),
            "http://127.0.0.1:5000/mcp/custom"
        );
    }

    #[test]
    fn parses_tools_list_payload() {
        let tools = parse_tools_list(&json!({
            "tools": [
                {
                    "name": "execute_sql",
                    "description": "Run SQL",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "sql": { "type": "string" } },
                        "required": ["sql"]
                    }
                }
            ]
        }))
        .unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "execute_sql");
        assert_eq!(tools[0].parameters["required"][0], "sql");
    }

    #[test]
    fn parses_call_text_as_json() {
        let v = parse_call_result(&json!({
            "content": [{ "type": "text", "text": "{\"rows\":1}" }],
            "isError": false
        }))
        .unwrap();
        assert_eq!(v["rows"], 1);
    }

    #[test]
    fn call_error_becomes_error() {
        let err = parse_call_result(&json!({
            "content": [{ "type": "text", "text": "relation missing" }],
            "isError": true
        }))
        .unwrap_err();
        assert!(err.to_string().contains("relation missing"));
    }
}
