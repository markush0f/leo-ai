use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::sleep;

use crate::host::workspace_root;

const DEFAULT_URL: &str = "http://127.0.0.1:8790";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppDto {
    pub ok: bool,
    pub phase: String,
    pub user: Option<String>,
    pub qr: Option<String>,
    pub allow_phones: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct ControlStatus {
    phase: String,
    user: Option<String>,
    qr: Option<String>,
    allow_phones: Vec<String>,
}

pub fn control_url() -> String {
    std::env::var("WHATSAPP_CONTROL_URL")
        .ok()
        .map(|raw| raw.trim().trim_end_matches('/').to_string())
        .filter(|raw| !raw.is_empty())
        .unwrap_or_else(|| DEFAULT_URL.to_string())
}

pub async fn status() -> WhatsAppDto {
    match get_status().await {
        Ok(dto) => dto,
        Err(error) => down(error),
    }
}

pub async fn pair() -> WhatsAppDto {
    match post("/pair").await {
        Ok(dto) => dto,
        Err(error) => down(error),
    }
}

pub async fn set_allow(phones: &str) -> WhatsAppDto {
    match put_allow(phones).await {
        Ok(dto) => dto,
        Err(error) => down(error),
    }
}

pub async fn start() -> WhatsAppDto {
    if let Ok(dto) = get_status().await {
        return dto;
    }
    let Some(root) = workspace_root() else {
        return down("no encuentro el repo (pon IRA_ROOT)");
    };
    let dir = root.join("services/ira-whatsapp");
    if !dir.join("package.json").is_file() {
        return down("falta services/ira-whatsapp");
    }
    if !dir.join("node_modules").is_dir() {
        return down("falta npm install en services/ira-whatsapp");
    }
    let child = Command::new("npm")
        .args(["start"])
        .current_dir(&dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(err) => return down(format!("no se pudo arrancar npm: {err}")),
    };
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    for _ in 0..20 {
        if let Ok(dto) = get_status().await {
            return dto;
        }
        sleep(Duration::from_millis(250)).await;
    }
    down("el puente no respondió. mira la terminal de ira-whatsapp")
}

fn down(error: impl Into<String>) -> WhatsAppDto {
    WhatsAppDto {
        ok: false,
        phase: "down".into(),
        user: None,
        qr: None,
        allow_phones: Vec::new(),
        error: Some(error.into()),
    }
}

fn from_control(status: ControlStatus) -> WhatsAppDto {
    WhatsAppDto {
        ok: true,
        phase: status.phase,
        user: status.user,
        qr: status.qr,
        allow_phones: status.allow_phones,
        error: None,
    }
}

async fn get_status() -> Result<WhatsAppDto, String> {
    let url = format!("{}/status", control_url());
    let response = client()
        .get(url)
        .send()
        .await
        .map_err(|_| "el puente de WhatsApp no está arrancado".to_string())?;
    parse(response).await
}

async fn post(path: &str) -> Result<WhatsAppDto, String> {
    let url = format!("{}{path}", control_url());
    let response = client()
        .post(url)
        .send()
        .await
        .map_err(|_| "el puente de WhatsApp no está arrancado".to_string())?;
    parse(response).await
}

async fn put_allow(phones: &str) -> Result<WhatsAppDto, String> {
    let url = format!("{}/allow", control_url());
    let response = client()
        .put(url)
        .json(&serde_json::json!({ "phones": phones }))
        .send()
        .await
        .map_err(|_| "el puente de WhatsApp no está arrancado".to_string())?;
    parse(response).await
}

async fn parse(response: reqwest::Response) -> Result<WhatsAppDto, String> {
    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(clip(&body));
    }
    let status: ControlStatus = response
        .json()
        .await
        .map_err(|_| "respuesta de WhatsApp ilegible".to_string())?;
    Ok(from_control(status))
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn clip(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() {
        return "el puente de WhatsApp falló".into();
    }
    if t.chars().count() > 240 {
        format!("{}…", t.chars().take(240).collect::<String>())
    } else {
        t.to_string()
    }
}
