//! Local Docker Compose services Ira can start and stop.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;

const BUILTIN: &[(&str, &str, bool)] = &[
    ("postgres", "Postgres", true),
    ("toolbox", "Toolbox", true),
    ("ira-realtime", "Realtime", false),
    ("colibri", "Colibrì", false),
    ("veritas-kanban", "Veritas", false),
    ("veritas-mcp", "Veritas MCP", false),
];

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub required: bool,
}

const REQUIRED: &[&str] = &["postgres", "toolbox"];
const DISCOVER_TTL: Duration = Duration::from_secs(20);

struct DiscoverCache {
    at: Instant,
    entries: Vec<CatalogEntry>,
}

static DISCOVER_CACHE: Mutex<Option<DiscoverCache>> = Mutex::new(None);

pub async fn discover_catalog() -> Result<Vec<CatalogEntry>, String> {
    if let Some(entries) = cached_catalog() {
        return Ok(entries);
    }
    let entries = load_catalog().await?;
    if let Ok(mut cache) = DISCOVER_CACHE.lock() {
        *cache = Some(DiscoverCache {
            at: Instant::now(),
            entries: entries.clone(),
        });
    }
    Ok(entries)
}

fn cached_catalog() -> Option<Vec<CatalogEntry>> {
    let cache = DISCOVER_CACHE.lock().ok()?;
    let hit = cache.as_ref()?;
    (hit.at.elapsed() < DISCOVER_TTL).then(|| hit.entries.clone())
}

async fn load_catalog() -> Result<Vec<CatalogEntry>, String> {
    parse_catalog(&compose_config().await?)
}

async fn compose_profiles() -> Vec<String> {
    let Ok(text) = compose_stdout(&["compose", "config", "--profiles"]).await else {
        return vec!["kanban".into()];
    };
    let profiles: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    if profiles.is_empty() {
        vec!["kanban".into()]
    } else {
        profiles
    }
}

fn parse_catalog(body: &str) -> Result<Vec<CatalogEntry>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "compose config ilegible".to_string())?;
    let Some(services) = value.get("services").and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    for (id, service) in services {
        if !safe_id(id) || !include_service(service) {
            continue;
        }
        let labels = labels_of(service);
        let name = labels
            .iter()
            .find(|(key, _)| key == "ira.name")
            .map(|(_, value)| value.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| display_name(id));
        let required = labels
            .iter()
            .find(|(key, _)| key == "ira.required")
            .map(|(_, value)| matches!(value.as_str(), "true" | "1" | "yes"))
            .unwrap_or_else(|| REQUIRED.contains(&id.as_str()));
        entries.push(CatalogEntry {
            id: id.clone(),
            name,
            required,
        });
    }
    entries.sort_by(|a, b| b.required.cmp(&a.required).then_with(|| a.id.cmp(&b.id)));
    Ok(entries)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpEndpoint {
    pub id: String,
    pub url: String,
}

pub async fn discover_mcp() -> Result<Vec<McpEndpoint>, String> {
    if let Some(entries) = cached_mcp() {
        return Ok(entries);
    }
    let body = compose_config().await?;
    let entries = parse_mcp(&body)?;
    if let Ok(mut cache) = MCP_CACHE.lock() {
        *cache = Some(DiscoverMcp {
            at: Instant::now(),
            entries: entries.clone(),
        });
    }
    Ok(entries)
}

fn cached_mcp() -> Option<Vec<McpEndpoint>> {
    let cache = MCP_CACHE.lock().ok()?;
    let hit = cache.as_ref()?;
    (hit.at.elapsed() < DISCOVER_TTL).then(|| hit.entries.clone())
}

struct DiscoverMcp {
    at: Instant,
    entries: Vec<McpEndpoint>,
}

static MCP_CACHE: Mutex<Option<DiscoverMcp>> = Mutex::new(None);

async fn compose_config() -> Result<String, String> {
    let profiles = compose_profiles().await;
    let mut args = vec!["compose".to_string()];
    for profile in &profiles {
        args.push("--profile".into());
        args.push(profile.clone());
    }
    args.push("config".into());
    args.push("--format".into());
    args.push("json".into());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    compose_stdout(&refs).await
}

fn parse_mcp(body: &str) -> Result<Vec<McpEndpoint>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "compose config ilegible".to_string())?;
    let Some(services) = value.get("services").and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    for (id, service) in services {
        if !safe_id(id) || !is_mcp(id, service) {
            continue;
        }
        let Some(url) = mcp_url(service) else {
            continue;
        };
        entries.push(McpEndpoint {
            id: id.clone(),
            url,
        });
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(entries)
}

fn is_mcp(id: &str, service: &Value) -> bool {
    match label(service, "ira.mcp").as_deref() {
        Some("false" | "0" | "no") => return false,
        Some(value) if value.starts_with("http") => return true,
        Some("true" | "1" | "yes") => return true,
        _ => {}
    }
    id.ends_with("-mcp") || healthcheck_text(service).contains("/mcp")
}

fn mcp_url(service: &Value) -> Option<String> {
    if let Some(value) = label(service, "ira.mcp")
        && value.starts_with("http")
    {
        return Some(value.trim_end_matches('/').to_string());
    }
    let ports = service.get("ports")?.as_array()?;
    let published = ports.iter().find_map(|port| {
        let value = port.get("published")?;
        value
            .as_str()
            .map(str::to_string)
            .or_else(|| value.as_u64().map(|port| port.to_string()))
    })?;
    Some(format!("http://127.0.0.1:{published}"))
}

fn healthcheck_text(service: &Value) -> String {
    service
        .get("healthcheck")
        .and_then(|check| check.get("test"))
        .map(|test| test.to_string())
        .unwrap_or_default()
}

fn include_service(service: &Value) -> bool {
    match label(service, "ira.catalog").as_deref() {
        Some("false" | "0" | "no") => false,
        Some("true" | "1" | "yes") => true,
        _ => long_running(service),
    }
}

fn long_running(service: &Value) -> bool {
    let restart = service.get("restart").and_then(Value::as_str).unwrap_or("");
    if !restart.is_empty() && !restart.eq_ignore_ascii_case("no") {
        return true;
    }
    service
        .get("ports")
        .and_then(Value::as_array)
        .is_some_and(|ports| !ports.is_empty())
        || service.get("healthcheck").is_some_and(|value| !value.is_null())
}

fn label(service: &Value, key: &str) -> Option<String> {
    labels_of(service)
        .into_iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

fn labels_of(service: &Value) -> Vec<(String, String)> {
    match service.get("labels") {
        Some(Value::Object(map)) => map
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
            .collect(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|item| {
                let (key, value) = item.split_once('=')?;
                Some((key.to_string(), value.to_string()))
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn display_name(id: &str) -> String {
    id.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn builtin_catalog() -> Vec<CatalogEntry> {
    BUILTIN
        .iter()
        .map(|(id, name, required)| CatalogEntry {
            id: (*id).into(),
            name: (*name).into(),
            required: *required,
        })
        .collect()
}

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
    if let Ok(raw) = std::env::var("IRA_ROOT") {
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

pub async fn status(catalog: &[CatalogEntry]) -> ServicesDto {
    match compose_ps().await {
        Ok(rows) => from_rows(catalog, &rows, None),
        Err(error) => from_rows(catalog, &[], Some(error)),
    }
}

pub async fn start(catalog: &[CatalogEntry]) -> ServicesDto {
    finish(
        catalog,
        compose(
            &[
                "compose",
                "--profile",
                "kanban",
                "up",
                "-d",
                "postgres",
                "toolbox",
            ],
            Duration::from_secs(180),
        )
        .await,
    )
    .await
}

pub async fn start_one(id: &str, catalog: &[CatalogEntry]) -> ServicesDto {
    let Ok(id) = known(id, catalog) else {
        return failed(catalog, "servicio desconocido");
    };
    let timeout = if id.starts_with("veritas-") {
        Duration::from_secs(900)
    } else {
        Duration::from_secs(180)
    };
    finish(
        catalog,
        compose(&["compose", "--profile", "kanban", "up", "-d", id], timeout).await,
    )
    .await
}

pub async fn stop_one(id: &str, catalog: &[CatalogEntry]) -> ServicesDto {
    let Ok(id) = known(id, catalog) else {
        return failed(catalog, "servicio desconocido");
    };
    finish(
        catalog,
        compose(
            &["compose", "--profile", "kanban", "stop", id],
            Duration::from_secs(60),
        )
        .await,
    )
    .await
}

fn known<'a>(id: &str, catalog: &'a [CatalogEntry]) -> Result<&'a str, ()> {
    if !safe_id(id) {
        return Err(());
    }
    catalog
        .iter()
        .find(|service| service.id == id)
        .map(|service| service.id.as_str())
        .ok_or(())
}

fn safe_id(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit())
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn failed(catalog: &[CatalogEntry], error: impl Into<String>) -> ServicesDto {
    let mut out = from_rows(catalog, &[], Some(error.into()));
    out.ok = false;
    out
}

async fn finish(catalog: &[CatalogEntry], result: Result<(), String>) -> ServicesDto {
    let mut out = status(catalog).await;
    if let Err(error) = result {
        out.ok = false;
        out.error = Some(error);
    }
    out
}

async fn compose(args: &[&str], timeout: Duration) -> Result<(), String> {
    let Some(root) = workspace_root() else {
        return Err("no encuentro docker-compose.yml (pon IRA_ROOT)".into());
    };
    let output = docker_command()
        .args(args)
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    let output = match tokio::time::timeout(timeout, output).await {
        Ok(Ok(out)) => out,
        Ok(Err(err)) => return Err(format!("no se pudo ejecutar docker: {err}")),
        Err(_) => return Err("docker compose tardó demasiado".into()),
    };
    if output.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    let msg = [err.trim(), out.trim()]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or("docker compose falló");
    Err(clip(msg))
}

async fn compose_stdout(args: &[&str]) -> Result<String, String> {
    let root = workspace_root().ok_or_else(|| "no encuentro docker-compose.yml".to_string())?;
    let output = docker_command()
        .args(args)
        .current_dir(&root)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|err| format!("no se pudo ejecutar docker: {err}"))?;
    if !output.status.success() {
        return Err(clip(&String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn compose_ps() -> Result<Vec<ComposeRow>, String> {
    let root = workspace_root().ok_or_else(|| {
        "no encuentro docker-compose.yml (pon IRA_ROOT al raíz del repo)".to_string()
    })?;
    let output = docker_command()
        .args(["compose", "--profile", "kanban", "ps", "--format", "json"])
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
    if std::env::var_os("IRA_UID").is_none()
        && let Some(uid) = process_id("-u")
    {
        command.env("IRA_UID", uid);
    }
    if std::env::var_os("IRA_GID").is_none()
        && let Some(gid) = process_id("-g")
    {
        command.env("IRA_GID", gid);
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

fn from_rows(catalog: &[CatalogEntry], rows: &[ComposeRow], error: Option<String>) -> ServicesDto {
    let services: Vec<ServiceDto> = catalog
        .iter()
        .map(|service| {
            let row = rows.iter().find(|r| r.service == service.id);
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
                        id: service.id.clone(),
                        name: service.name.clone(),
                        running,
                        healthy,
                        detail,
                    }
                }
                None => ServiceDto {
                    id: service.id.clone(),
                    name: service.name.clone(),
                    running: false,
                    healthy: false,
                    detail: "parado".into(),
                },
            }
        })
        .collect();
    let ok = error.is_none()
        && catalog
            .iter()
            .filter(|service| service.required)
            .all(|need| {
                services
                    .iter()
                    .any(|service| service.id == need.id && service.healthy)
            });
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
    fn mcp_endpoint_comes_from_published_port() {
        let raw = r#"{
            "services": {
                "toolbox": {"image": "toolbox", "ports": [{"published": "5000"}]},
                "veritas-mcp": {"image": "mcp", "ports": [{"published": "3100"}], "healthcheck": {"test": ["CMD", "fetch('/mcp')"]}},
                "custom": {"image": "x", "labels": {"ira.mcp": "http://127.0.0.1:9"}}
            }
        }"#;
        let entries = parse_mcp(raw).unwrap();
        assert_eq!(
            entries,
            vec![
                McpEndpoint {
                    id: "custom".into(),
                    url: "http://127.0.0.1:9".into(),
                },
                McpEndpoint {
                    id: "veritas-mcp".into(),
                    url: "http://127.0.0.1:3100".into(),
                },
            ]
        );
    }

    #[test]
    fn catalog_skips_one_shot_and_keeps_profile_services() {
        let raw = r#"{
            "services": {
                "postgres-migrate": {"image": "postgres:16"},
                "postgres": {"image": "postgres:16", "ports": [{"target": 5432}], "healthcheck": {"test": ["CMD", "true"]}},
                "toolbox": {"image": "toolbox", "restart": "unless-stopped"},
                "veritas-kanban": {"image": "veritas", "restart": "unless-stopped", "profiles": ["kanban"]},
                "hidden": {"image": "x", "restart": "unless-stopped", "labels": {"ira.catalog": "false"}},
                "named": {"image": "x", "ports": [{"target": 1}], "labels": ["ira.name=Bonito", "ira.required=true"]}
            }
        }"#;
        let entries = parse_catalog(raw).unwrap();
        let ids: Vec<_> = entries.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(
            ids,
            ["named", "postgres", "toolbox", "veritas-kanban"]
        );
        assert!(entries.iter().any(|entry| entry.id == "named" && entry.name == "Bonito" && entry.required));
        assert!(entries.iter().any(|entry| entry.id == "veritas-kanban" && !entry.required));
    }

    #[test]
    fn parses_ndjson_compose_ps() {
        let raw = r#"{"Service":"postgres","State":"running","Health":"healthy","Status":"Up (healthy)"}
{"Service":"toolbox","State":"running","Health":"healthy","Status":"Up (healthy)"}"#;
        let dto = from_rows(&builtin_catalog(), &parse_ps(raw), None);
        assert!(dto.ok);
        assert_eq!(dto.services[0].id, "postgres");
        assert!(dto.services[0].healthy);
        assert_eq!(dto.services[1].id, "toolbox");
    }

    #[test]
    fn missing_service_is_down() {
        let raw = r#"{"Service":"postgres","State":"running","Health":"healthy","Status":"Up"}"#;
        let dto = from_rows(&builtin_catalog(), &parse_ps(raw), None);
        assert!(!dto.ok);
        assert!(dto.services[0].healthy);
        assert!(!dto.services[1].running);
        assert_eq!(dto.services[1].detail, "parado");
        assert!(
            dto.services
                .iter()
                .any(|s| s.id == "veritas-kanban" && !s.running)
        );
    }

    #[test]
    fn optional_service_down_does_not_block_core() {
        let raw = r#"{"Service":"postgres","State":"running","Health":"healthy","Status":"Up"}
{"Service":"toolbox","State":"running","Health":"healthy","Status":"Up"}"#;
        let dto = from_rows(&builtin_catalog(), &parse_ps(raw), None);
        assert!(dto.ok);
        assert!(
            dto.services
                .iter()
                .any(|s| s.id == "veritas-mcp" && !s.healthy)
        );
    }

    #[test]
    fn unknown_service_is_rejected() {
        let catalog = builtin_catalog();
        assert!(known("postgres", &catalog).is_ok());
        assert!(known("veritas-kanban", &catalog).is_ok());
        assert!(known("rm-rf", &catalog).is_err());
        assert!(known("postgres;drop", &catalog).is_err());
    }

    #[test]
    fn parses_json_array() {
        let raw = r#"[{"Service":"postgres","State":"exited","Health":"","Status":"Exited"}]"#;
        let dto = from_rows(&builtin_catalog(), &parse_ps(raw), None);
        assert!(!dto.services[0].running);
        assert!(!dto.ok);
    }
}
