//! Local Docker Compose services Leo needs (Postgres and MCP Toolbox).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

const WANTED: &[(&str, &str)] = &[("postgres", "Postgres"), ("toolbox", "Toolbox")];

#[derive(Debug, Clone, Serialize)]
pub struct ServiceDto {
    pub id: String,
    pub name: String,
    pub running: bool,
    pub healthy: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServicesDto {
    pub ok: bool,
    pub services: Vec<ServiceDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ComposeRow {
    #[serde(default, alias = "Service")]
    service: String,
    #[serde(default, alias = "State")]
    state: String,
    #[serde(default, alias = "Health")]
    health: String,
    #[serde(default, alias = "Status")]
    status: String,
}

pub fn workspace_root() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("LEO_ROOT") {
        let p = PathBuf::from(raw.trim());
        if is_workspace(&p) {
            return Some(p);
        }
    }
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if is_workspace(&dir) {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn is_workspace(dir: &Path) -> bool {
    dir.join("docker-compose.yml").is_file() && dir.join("crates").is_dir()
}

pub async fn status() -> ServicesDto {
    match compose_ps().await {
        Ok(rows) => from_rows(&rows, None),
        Err(error) => from_rows(&[], Some(error)),
    }
}

pub async fn start() -> ServicesDto {
    let Some(root) = workspace_root() else {
        return from_rows(
            &[],
            Some("no encuentro docker-compose.yml (pon LEO_ROOT)".into()),
        );
    };
    let output = docker_command()
        .args(["compose", "up", "-d", "postgres", "toolbox"])
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    let output = match tokio::time::timeout(Duration::from_secs(180), output).await {
        Ok(Ok(out)) => out,
        Ok(Err(err)) => {
            return from_rows(&[], Some(format!("no se pudo ejecutar docker: {err}")));
        }
        Err(_) => {
            return from_rows(&[], Some("docker compose tardó más de 3 minutos".into()));
        }
    };
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        let msg = [err.trim(), out.trim()]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or("docker compose up falló");
        return from_rows(&[], Some(clip(msg)));
    }
    match compose_ps().await {
        Ok(rows) => from_rows(&rows, None),
        Err(error) => from_rows(&[], Some(error)),
    }
}

async fn compose_ps() -> Result<Vec<ComposeRow>, String> {
    let root = workspace_root().ok_or_else(|| {
        "no encuentro docker-compose.yml (pon LEO_ROOT al raíz del repo)".to_string()
    })?;
    let output = docker_command()
        .args(["compose", "ps", "--format", "json", "postgres", "toolbox"])
        .current_dir(&root)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|err| format!("no se pudo ejecutar docker: {err}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(clip(err.trim()));
    }
    Ok(parse_ps(&String::from_utf8_lossy(&output.stdout)))
}

fn docker_command() -> Command {
    let mut command = Command::new("docker");
    if std::env::var_os("LEO_UID").is_none()
        && let Some(uid) = process_id("-u")
    {
        command.env("LEO_UID", uid);
    }
    if std::env::var_os("LEO_GID").is_none()
        && let Some(gid) = process_id("-g")
    {
        command.env("LEO_GID", gid);
    }
    command
}

fn process_id(flag: &str) -> Option<String> {
    let output = std::process::Command::new("id")
        .arg(flag)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_ps(stdout: &str) -> Vec<ComposeRow> {
    let t = stdout.trim();
    if t.is_empty() {
        return Vec::new();
    }
    if t.starts_with('[') {
        return serde_json::from_str(t).unwrap_or_default();
    }
    t.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                None
            } else {
                serde_json::from_str(line).ok()
            }
        })
        .collect()
}

fn from_rows(rows: &[ComposeRow], error: Option<String>) -> ServicesDto {
    let services: Vec<ServiceDto> = WANTED
        .iter()
        .map(|(id, name)| {
            let row = rows.iter().find(|r| r.service == *id);
            match row {
                Some(row) => {
                    let running = row.state.eq_ignore_ascii_case("running");
                    let healthy = running
                        && (row.health.is_empty() || row.health.eq_ignore_ascii_case("healthy"));
                    let detail = if row.status.is_empty() {
                        row.state.clone()
                    } else {
                        row.status.clone()
                    };
                    ServiceDto {
                        id: (*id).into(),
                        name: (*name).into(),
                        running,
                        healthy,
                        detail,
                    }
                }
                None => ServiceDto {
                    id: (*id).into(),
                    name: (*name).into(),
                    running: false,
                    healthy: false,
                    detail: "parado".into(),
                },
            }
        })
        .collect();
    let ok = services.iter().all(|s| s.healthy);
    ServicesDto {
        ok,
        services,
        error,
    }
}

fn clip(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > 400 {
        format!("{}…", t.chars().take(400).collect::<String>())
    } else {
        t.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ndjson_compose_ps() {
        let raw = r#"{"Service":"postgres","State":"running","Health":"healthy","Status":"Up (healthy)"}
{"Service":"toolbox","State":"running","Health":"healthy","Status":"Up (healthy)"}"#;
        let dto = from_rows(&parse_ps(raw), None);
        assert!(dto.ok);
        assert_eq!(dto.services[0].id, "postgres");
        assert!(dto.services[0].healthy);
        assert_eq!(dto.services[1].id, "toolbox");
    }

    #[test]
    fn missing_service_is_down() {
        let raw = r#"{"Service":"postgres","State":"running","Health":"healthy","Status":"Up"}"#;
        let dto = from_rows(&parse_ps(raw), None);
        assert!(!dto.ok);
        assert!(dto.services[0].healthy);
        assert!(!dto.services[1].running);
        assert_eq!(dto.services[1].detail, "parado");
    }

    #[test]
    fn parses_json_array() {
        let raw = r#"[{"Service":"postgres","State":"exited","Health":"","Status":"Exited"}]"#;
        let dto = from_rows(&parse_ps(raw), None);
        assert!(!dto.services[0].running);
        assert!(!dto.ok);
    }
}
