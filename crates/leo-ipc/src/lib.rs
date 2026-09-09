use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("el daemon no está en marcha ({0})")]
    NotRunning(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Listen,
    Stop,
    Speak { text: String },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl Response {
    pub fn ok(state: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: true,
            state: state.into(),
            message: Some(message.into()),
        }
    }

    pub fn err(state: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            state: state.into(),
            message: Some(message.into()),
        }
    }
}

pub fn socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("leo-ai.sock");
    }
    PathBuf::from("/tmp/leo-ai.sock")
}

pub async fn bind(path: &Path) -> Result<UnixListener, IpcError> {
    let _ = std::fs::remove_file(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(UnixListener::bind(path)?)
}

pub async fn send(path: &Path, req: &Request) -> Result<Response, IpcError> {
    let stream = UnixStream::connect(path)
        .await
        .map_err(|_| IpcError::NotRunning(path.to_path_buf()))?;
    let (reader, mut writer) = stream.into_split();
    let mut line = serde_json::to_string(req)?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await?;
    writer.shutdown().await?;

    let mut buf = String::new();
    let mut reader = BufReader::new(reader);
    reader.read_line(&mut buf).await?;
    Ok(serde_json::from_str(buf.trim())?)
}

pub async fn read_request(stream: &mut UnixStream) -> Result<Request, IpcError> {
    let mut reader = BufReader::new(stream);
    let mut buf = String::new();
    reader.read_line(&mut buf).await?;
    Ok(serde_json::from_str(buf.trim())?)
}

pub async fn write_response(stream: &mut UnixStream, resp: &Response) -> Result<(), IpcError> {
    let mut line = serde_json::to_string(resp)?;
    line.push('\n');
    stream.write_all(line.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}
