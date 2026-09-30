use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::error::{Error, clip};
use crate::rpc::{self, PROTOCOL};

#[derive(Clone)]
pub struct HttpSession {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    url: String,
    headers: HashMap<String, String>,
    next_id: AtomicU64,
    session: Mutex<SessionState>,
}

struct SessionState {
    id: Option<String>,
    protocol: String,
    ready: bool,
}

impl HttpSession {
    pub fn new(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        let http = reqwest::Client::builder()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            inner: Arc::new(Inner {
                http,
                url: url.into(),
                headers,
                next_id: AtomicU64::new(1),
                session: Mutex::new(SessionState {
                    id: None,
                    protocol: PROTOCOL.to_string(),
                    ready: false,
                }),
            }),
        }
    }

    pub async fn ensure(&self, timeout: Duration) -> Result<(), Error> {
        {
            let state = self.inner.session.lock().await;
            if state.ready {
                return Ok(());
            }
        }
        let (headers, result) = self
            .rpc_raw(
                "initialize",
                rpc::initialize_params(),
                false,
                None,
                PROTOCOL,
                timeout,
            )
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
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        {
            let mut state = self.inner.session.lock().await;
            state.id = session_id.clone();
            state.protocol = protocol.clone();
            state.ready = false;
        }
        let _ = self
            .rpc_raw(
                "notifications/initialized",
                json!({}),
                true,
                session_id.as_deref(),
                &protocol,
                timeout,
            )
            .await?;
        let mut state = self.inner.session.lock().await;
        state.ready = true;
        Ok(())
    }

    pub async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, Error> {
        self.ensure(timeout).await?;
        let (id, protocol) = {
            let state = self.inner.session.lock().await;
            (state.id.clone(), state.protocol.clone())
        };
        self.rpc_raw(method, params, false, id.as_deref(), &protocol, timeout)
            .await?
            .1
            .ok_or_else(|| Error::msg(format!("{method} sin resultado")))
    }

    async fn rpc_raw(
        &self,
        method: &str,
        params: Value,
        notification: bool,
        session_id: Option<&str>,
        protocol: &str,
        timeout: Duration,
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
            .post(&self.inner.url)
            .timeout(timeout)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", protocol);
        for (key, value) in &self.inner.headers {
            if !key.trim().is_empty() {
                builder = builder.header(key, value);
            }
        }
        let mut builder = builder.json(&body);
        if let Some(sid) = session_id.filter(|value| !value.is_empty()) {
            builder = builder.header("Mcp-Session-Id", sid);
        }
        let response = builder.send().await?;
        let status = response.status();
        let headers = response.headers().clone();
        let content_type = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let text = response.text().await?;
        if notification
            && (status.as_u16() == 202 || status.as_u16() == 204 || text.trim().is_empty())
        {
            return Ok((headers, None));
        }
        if !status.is_success() && status.as_u16() != 202 {
            if let Ok(rpc) = rpc::decode_rpc(&content_type, &text)
                && let Some(err) = rpc::rpc_error(&rpc)
            {
                return Err(err);
            }
            return Err(Error::msg(format!(
                "http {}: {}",
                status.as_u16(),
                clip(&text)
            )));
        }
        if text.trim().is_empty() {
            return Ok((headers, None));
        }
        let decoded = rpc::decode_rpc(&content_type, &text)?;
        if let Some(err) = rpc::rpc_error(&decoded) {
            return Err(err);
        }
        Ok((headers, decoded.get("result").cloned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_remote_endpoint_path_exact() {
        let session = HttpSession::new("https://example.com/api/v2/tools", HashMap::new());
        assert_eq!(session.inner.url, "https://example.com/api/v2/tools");
    }
}
