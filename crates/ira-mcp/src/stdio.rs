use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;

use crate::error::Error;
use crate::rpc::{self, json_id};

pub struct StdioSession {
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, Error>>>>>,
    next_id: AtomicI64,
    child: Mutex<Child>,
    reader: JoinHandle<()>,
}

impl Drop for StdioSession {
    fn drop(&mut self) {
        self.reader.abort();
        if let Ok(mut child) = self.child.try_lock() {
            let _ = child.start_kill();
        }
    }
}

impl StdioSession {
    pub async fn spawn(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        timeout: Duration,
    ) -> Result<Self, Error> {
        let mut child = Command::new(command)
            .args(args)
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| Error::msg(format!("no se pudo lanzar `{command}`: {err}")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::msg("stdio sin stdout"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::msg("stdio sin stdin"))?;
        let stdin = Arc::new(Mutex::new(stdin));
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let reader = tokio::spawn(read_loop(stdout, stdin.clone(), pending.clone()));
        let session = Self {
            stdin,
            pending,
            next_id: AtomicI64::new(1),
            child: Mutex::new(child),
            reader,
        };
        let result = session
            .request("initialize", rpc::initialize_params(), timeout)
            .await?;
        let _ = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or(rpc::PROTOCOL);
        session
            .notify("notifications/initialized", json!({}))
            .await?;
        Ok(session)
    }

    pub async fn alive(&self) -> bool {
        match self.child.lock().await.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    }

    pub async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, Error> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        if let Err(err) = write_json(&self.stdin, &body).await {
            self.pending.lock().await.remove(&id);
            return Err(err);
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(err))) => Err(err),
            Ok(Err(_)) => Err(Error::msg(format!("{method}: sesión stdio cerrada"))),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(Error::Timeout)
            }
        }
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), Error> {
        write_json(
            &self.stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
            }),
        )
        .await
    }
}

async fn write_json(stdin: &Arc<Mutex<ChildStdin>>, value: &Value) -> Result<(), Error> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    let mut stdin = stdin.lock().await;
    stdin.write_all(&bytes).await?;
    stdin.flush().await?;
    Ok(())
}

async fn read_loop(
    stdout: tokio::process::ChildStdout,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, Error>>>>>,
) {
    let mut reader = BufReader::new(stdout);
    loop {
        match next_message(&mut reader).await {
            Ok(Some(message)) => dispatch(message, &stdin, &pending).await,
            Ok(None) => break,
            Err(err) => {
                fail_all(&pending, err).await;
                break;
            }
        }
    }
    fail_all(&pending, Error::msg("stdio cerrado")).await;
}

async fn dispatch(
    message: Value,
    stdin: &Arc<Mutex<ChildStdin>>,
    pending: &Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, Error>>>>>,
) {
    let id = json_id(&message);
    let method = message.get("method").and_then(Value::as_str);
    match (method, id) {
        (Some(method), Some(id)) => {
            let body = json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("método no soportado: {method}") },
            });
            let _ = write_json(stdin, &body).await;
        }
        (None, Some(id)) => {
            let reply = if let Some(err) = rpc::rpc_error(&message) {
                Err(err)
            } else {
                Ok(message.get("result").cloned().unwrap_or(Value::Null))
            };
            if let Some(tx) = pending.lock().await.remove(&id) {
                let _ = tx.send(reply);
            }
        }
        _ => {}
    }
}

async fn fail_all(
    pending: &Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, Error>>>>>,
    err: Error,
) {
    let waiters = std::mem::take(&mut *pending.lock().await);
    for (_, tx) in waiters {
        let _ = tx.send(Err(Error::msg(err.to_string())));
    }
}

async fn next_message(reader: &mut BufReader<tokio::process::ChildStdout>) -> Result<Option<Value>, Error> {
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(None);
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Box::pin(next_message(reader)).await;
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Ok(Some(serde_json::from_str(trimmed)?));
    }
    if trimmed.to_ascii_lowercase().starts_with("content-length:") {
        let len: usize = trimmed
            .split_once(':')
            .and_then(|(_, value)| value.trim().parse().ok())
            .ok_or_else(|| Error::msg("Content-Length inválido"))?;
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).await? == 0 {
                return Ok(None);
            }
            if header.trim().is_empty() {
                break;
            }
        }
        let mut buf = vec![0_u8; len];
        tokio::io::AsyncReadExt::read_exact(reader, &mut buf).await?;
        return Ok(Some(serde_json::from_slice(&buf)?));
    }
    Err(Error::msg(format!("trama stdio ilegible: {trimmed}")))
}
