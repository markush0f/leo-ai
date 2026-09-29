//! Local Docker Compose services Ira can start and stop.

use std::collections::BTreeMap;
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
    ("projects-api", "Projects", false),
    ("projects-mcp", "Projects MCP", false),
    ("ira-gateway", "Services", false),
];

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub required: bool,
    pub host_port: Option<u16>,
    pub container_port: Option<u16>,
    pub kind: String,
    pub description: String,
    pub peer: Option<String>,
}

const GATEWAY_UPSTREAMS: &[&str] = &["ira-realtime", "projects-api", "projects-mcp"];

fn default_gateway_port() -> u16 {
    8790
}

fn default_autostart() -> Vec<String> {
    vec!["postgres".into(), "toolbox".into(), "ira-gateway".into()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct HostFile {
    gateway_port: u16,
    ports: BTreeMap<String, u16>,
    autostart: Vec<String>,
    #[serde(default)]
    meta: BTreeMap<String, ServiceMeta>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ServiceMeta {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
}

impl Default for HostFile {
    fn default() -> Self {
        Self {
            gateway_port: default_gateway_port(),
            ports: BTreeMap::new(),
            autostart: default_autostart(),
            meta: BTreeMap::new(),
        }
    }
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
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_catalog(body: &str) -> Result<Vec<CatalogEntry>, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| "compose config ilegible".to_string())?;
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
        let (host_port, container_port) = ports_of(service);
        let kind = if is_mcp(id, service) { "mcp" } else { "service" };
        let description = label(service, "ira.description")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_description(id).to_string());
        let peer = label(service, "ira.peer")
            .filter(|value| safe_id(value))
            .or_else(|| default_peer(id).map(str::to_string));
        entries.push(CatalogEntry {
            id: id.clone(),
            name,
            required,
            host_port,
            container_port,
            kind: kind.into(),
            description,
            peer,
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
    let value: Value =
        serde_json::from_str(body).map_err(|_| "compose config ilegible".to_string())?;
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
        || service
            .get("healthcheck")
            .is_some_and(|value| !value.is_null())
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
            host_port: None,
            container_port: None,
            kind: if id.ends_with("-mcp") { "mcp" } else { "service" }.into(),
            description: default_description(id).into(),
            peer: default_peer(id).map(str::to_string),
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
    pub autostart: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_port: Option<u16>,
    pub via_gateway: bool,
    pub kind: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServicesDto {
    pub ok: bool,
    pub services: Vec<ServiceDto>,
    pub gateway_port: u16,
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
    let cfg = load_host_file();
    let marked: Vec<&str> = cfg
        .autostart
        .iter()
        .filter_map(|id| known(id, catalog).ok())
        .collect();
    let mut selected = Vec::new();
    for id in marked {
        for item in unit(id, catalog) {
            if !selected.contains(&item) {
                selected.push(item);
            }
        }
    }
    if selected.is_empty() {
        return failed(catalog, "ningún servicio marcado para el arranque");
    }
    let (slow, fast): (Vec<&str>, Vec<&str>) = selected.into_iter().partition(|id| {
        id.starts_with("projects-") || *id == "ira-gateway"
    });
    if let Err(error) = compose_up(&fast, Duration::from_secs(180)).await {
        return finish(catalog, Err(error)).await;
    }
    finish(catalog, compose_up(&slow, Duration::from_secs(900)).await).await
}

pub fn set_boot(id: &str, on: bool, catalog: &[CatalogEntry]) -> Result<(), String> {
    let id = known(id, catalog)
        .map_err(|()| "servicio desconocido".to_string())?
        .to_string();
    let mut cfg = load_host_file();
    let ids = unit(&id, catalog)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if on {
        for item in ids {
            if !cfg.autostart.iter().any(|saved| saved == &item) {
                cfg.autostart.push(item);
            }
        }
    } else {
        cfg.autostart.retain(|item| !ids.iter().any(|id| id == item));
    }
    save_host_file(&cfg)
}

pub async fn set_port(id: &str, port: u16, catalog: &[CatalogEntry]) -> ServicesDto {
    if !(1..=65535).contains(&port) {
        return failed(catalog, "puerto fuera de rango");
    }
    let Ok(id) = known(id, catalog) else {
        return failed(catalog, "servicio desconocido");
    };
    if GATEWAY_UPSTREAMS.contains(&id) {
        return failed(
            catalog,
            "ese servicio entra por el gateway; cambia el puerto del gateway",
        );
    }
    let publishes = id == "ira-gateway" || port_env(id).is_some();
    if !publishes {
        return failed(catalog, "ese servicio no publica puerto de host");
    }
    let mut cfg = load_host_file();
    if id == "ira-gateway" {
        cfg.gateway_port = port;
    } else {
        cfg.ports.insert(id.to_string(), port);
    }
    if let Err(error) = save_host_file(&cfg) {
        return failed(catalog, error);
    }
    publish_env(id, port);
    finish(catalog, compose_up(&[id], Duration::from_secs(180)).await).await
}

pub async fn start_one(id: &str, catalog: &[CatalogEntry]) -> ServicesDto {
    let Ok(id) = known(id, catalog) else {
        return failed(catalog, "servicio desconocido");
    };
    let ids = start_order(&unit(id, catalog), catalog);
    let timeout = if ids.iter().any(|item| item.starts_with("projects-") || *item == "ira-gateway") {
        Duration::from_secs(900)
    } else {
        Duration::from_secs(180)
    };
    finish(catalog, compose_up(&ids, timeout).await).await
}

pub async fn stop_one(id: &str, catalog: &[CatalogEntry]) -> ServicesDto {
    let Ok(id) = known(id, catalog) else {
        return failed(catalog, "servicio desconocido");
    };
    let mut ids = start_order(&unit(id, catalog), catalog);
    ids.reverse();
    let mut args = Vec::with_capacity(ids.len() + 2);
    args.push("compose");
    args.push("stop");
    args.extend(ids);
    finish(catalog, compose(&args, Duration::from_secs(60)).await).await
}

pub fn set_meta(id: &str, name: &str, description: &str, catalog: &[CatalogEntry]) -> Result<(), String> {
    let id = known(id, catalog)
        .map_err(|()| "servicio desconocido".to_string())?
        .to_string();
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("el nombre debe tener entre 1 y 80 caracteres".into());
    }
    let description = description.trim();
    if description.chars().count() > 400 {
        return Err("la descripción supera 400 caracteres".into());
    }
    let mut cfg = load_host_file();
    cfg.meta.insert(
        id,
        ServiceMeta {
            name: name.to_string(),
            description: description.to_string(),
        },
    );
    save_host_file(&cfg)
}

pub fn catalog_note(catalog: &[CatalogEntry]) -> String {
    let cfg = load_host_file();
    let mut lines = Vec::new();
    for service in catalog {
        let (name, description) = display_meta(&cfg, &service.id, &service.name, &service.description);
        if description.trim().is_empty() {
            continue;
        }
        let kind = if service.kind == "mcp" { "MCP" } else { "servicio" };
        lines.push(format!("- {name} ({kind}): {description}"));
    }
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "Servicios locales. Elige el que coincida con la petición; un MCP y su servicio local van juntos:\n{}",
        lines.join("\n")
    )
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

async fn compose_up(ids: &[&str], timeout: Duration) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut args = Vec::with_capacity(ids.len() + 3);
    args.push("compose");
    args.push("up");
    args.push("-d");
    args.extend(ids.iter().copied());
    compose(&args, timeout).await
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
        .args(["compose", "ps", "--format", "json"])
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
    let cfg = load_host_file();
    command.env("IRA_SERVICES_PORT", cfg.gateway_port.to_string());
    for (id, port) in &cfg.ports {
        if let Some(key) = port_env(id) {
            command.env(key, port.to_string());
        }
    }
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
    let cfg = load_host_file();
    let services: Vec<ServiceDto> = catalog
        .iter()
        .map(|service| {
            let row = rows.iter().find(|r| r.service == service.id);
            let (running, healthy, detail) = match row {
                Some(row) => {
                    let running = row.state.eq_ignore_ascii_case("running");
                    let healthy = running
                        && (row.health.is_empty() || row.health.eq_ignore_ascii_case("healthy"));
                    let detail = if row.status.is_empty() {
                        row.state.clone()
                    } else {
                        row.status.clone()
                    };
                    (running, healthy, detail)
                }
                None => (false, false, "parado".into()),
            };
            let via_gateway = GATEWAY_UPSTREAMS.contains(&service.id.as_str());
            let (name, description) = display_meta(&cfg, &service.id, &service.name, &service.description);
            ServiceDto {
                id: service.id.clone(),
                name,
                running,
                healthy,
                detail,
                autostart: cfg.autostart.iter().any(|id| id == &service.id),
                host_port: if via_gateway {
                    None
                } else {
                    published_port(&service.id, service, &cfg)
                },
                container_port: service.container_port.or_else(|| internal_port(&service.id)),
                via_gateway,
                kind: service.kind.clone(),
                description,
                peer: service.peer.clone(),
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
        gateway_port: cfg.gateway_port,
        error,
    }
}

fn unit<'a>(id: &str, catalog: &'a [CatalogEntry]) -> Vec<&'a str> {
    let Some(entry) = catalog.iter().find(|service| service.id == id) else {
        return Vec::new();
    };
    let mut ids = vec![entry.id.as_str()];
    if let Some(peer) = entry.peer.as_deref()
        && catalog.iter().any(|service| service.id == peer)
        && !ids.contains(&peer)
    {
        ids.push(peer);
    }
    ids
}

fn start_order<'a>(ids: &[&'a str], catalog: &[CatalogEntry]) -> Vec<&'a str> {
    let mut services = Vec::new();
    let mut mcps = Vec::new();
    for id in ids {
        let mcp = catalog
            .iter()
            .find(|service| service.id == *id)
            .is_some_and(|service| service.kind == "mcp");
        if mcp {
            mcps.push(*id);
        } else {
            services.push(*id);
        }
    }
    services.extend(mcps);
    services
}

fn display_meta(cfg: &HostFile, id: &str, name: &str, description: &str) -> (String, String) {
    match cfg.meta.get(id) {
        Some(meta) if !meta.name.trim().is_empty() => (
            meta.name.trim().to_string(),
            if meta.description.trim().is_empty() {
                description.to_string()
            } else {
                meta.description.trim().to_string()
            },
        ),
        Some(meta) => (name.to_string(), meta.description.trim().to_string()),
        None => (name.to_string(), description.to_string()),
    }
}

fn default_peer(id: &str) -> Option<&'static str> {
    match id {
        "projects-api" => Some("projects-mcp"),
        "projects-mcp" => Some("projects-api"),
        _ => None,
    }
}

fn default_description(id: &str) -> &'static str {
    match id {
        "postgres" => "Base de datos de Ira en este equipo.",
        "toolbox" => "SQL y esquema de bases de datos. No sirve para proyectos.",
        "ira-realtime" => "Voz en tiempo real.",
        "colibri" => "Modelo local Colibrì.",
        "ira-gateway" => "Puerto único hacia los servicios locales que no publican el suyo.",
        "projects-api" => "Servicio local de proyectos y tareas. Los datos viven en este equipo.",
        "projects-mcp" => "MCP local de proyectos. Úsalo para listar, crear y editar proyectos y tareas de este equipo.",
        _ => "",
    }
}

fn published_port(id: &str, entry: &CatalogEntry, cfg: &HostFile) -> Option<u16> {
    if id == "ira-gateway" {
        return Some(cfg.gateway_port);
    }
    cfg.ports
        .get(id)
        .copied()
        .or(entry.host_port)
        .or_else(|| default_host_port(id))
}

fn default_host_port(id: &str) -> Option<u16> {
    match id {
        "postgres" => Some(5439),
        "toolbox" => Some(5000),
        "colibri" => Some(8081),
        "ira-gateway" => Some(default_gateway_port()),
        _ => None,
    }
}

fn internal_port(id: &str) -> Option<u16> {
    match id {
        "postgres" => Some(5432),
        "toolbox" | "colibri" => Some(5000),
        "ira-gateway" => Some(8790),
        "ira-realtime" => Some(8765),
        "projects-api" => Some(3200),
        "projects-mcp" => Some(3201),
        _ => None,
    }
}

fn port_env(id: &str) -> Option<&'static str> {
    match id {
        "ira-gateway" => Some("IRA_SERVICES_PORT"),
        "postgres" => Some("POSTGRES_PORT"),
        "toolbox" => Some("TOOLBOX_PORT"),
        "colibri" => Some("COLIBRI_PORT"),
        _ => None,
    }
}

fn ports_of(service: &Value) -> (Option<u16>, Option<u16>) {
    let Some(port) = service.get("ports").and_then(Value::as_array).and_then(|ports| ports.first())
    else {
        return (None, None);
    };
    match port {
        Value::Object(map) => (
            map.get("published").and_then(parse_port_value),
            map.get("target").and_then(parse_port_value),
        ),
        Value::String(raw) => parse_port_mapping(raw),
        _ => (None, None),
    }
}

fn parse_port_value(value: &Value) -> Option<u16> {
    value
        .as_u64()
        .and_then(|port| u16::try_from(port).ok())
        .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
}

fn parse_port_mapping(raw: &str) -> (Option<u16>, Option<u16>) {
    let spec = raw.rsplit('/').next().unwrap_or(raw);
    let mut parts = spec.rsplit(':');
    let container = parts.next().and_then(|part| part.parse().ok());
    let host = parts.next().and_then(|part| part.parse().ok());
    (host.or(container), container)
}

fn host_file_path() -> Option<PathBuf> {
    workspace_root().map(|root| root.join(".ira").join("host.json"))
}

fn load_host_file() -> HostFile {
    let Some(path) = host_file_path() else {
        return HostFile::default();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return HostFile::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_host_file(cfg: &HostFile) -> Result<(), String> {
    let path = host_file_path().ok_or_else(|| "no encuentro el raíz del repo".to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| format!("no se pudo crear .ira: {err}"))?;
    }
    let body = serde_json::to_string_pretty(cfg).map_err(|err| err.to_string())?;
    std::fs::write(&path, body).map_err(|err| format!("no se pudo guardar el arranque: {err}"))
}

fn publish_env(id: &str, port: u16) {
    let Some(key) = port_env(id) else {
        return;
    };
    unsafe { std::env::set_var(key, port.to_string()) };
    upsert_dotenv(key, &port.to_string());
    match id {
        "postgres" => {
            rewrite_url_env("DATABASE_URL", port);
            rewrite_url_env("IRA_DATABASE_URL", port);
            upsert_url_dotenv("DATABASE_URL", port);
            upsert_url_dotenv("IRA_DATABASE_URL", port);
        }
        "toolbox" => {
            rewrite_url_env("MCP_TOOLBOX_URL", port);
            upsert_url_dotenv("MCP_TOOLBOX_URL", port);
        }
        _ => {}
    }
}

fn rewrite_url_env(key: &str, port: u16) {
    let Ok(url) = std::env::var(key) else {
        return;
    };
    if let Some(next) = replace_port_in_url(&url, port) {
        unsafe { std::env::set_var(key, next) };
    }
}

fn upsert_dotenv(key: &str, value: &str) {
    let Some(path) = workspace_root().map(|root| root.join(".env")) else {
        return;
    };
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut found = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with(&format!("{key}=")) || trimmed.starts_with(&format!("#{key}="))
            {
                found = true;
                format!("{key}={value}")
            } else {
                line.to_string()
            }
        })
        .collect();
    if !found {
        if !lines.is_empty() && !lines.last().is_some_and(|line| line.is_empty()) {
            lines.push(String::new());
        }
        lines.push(format!("{key}={value}"));
    }
    let mut body = lines.join("\n");
    body.push('\n');
    let _ = std::fs::write(path, body);
}

fn upsert_url_dotenv(key: &str, port: u16) {
    let Some(path) = workspace_root().map(|root| root.join(".env")) else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut changed = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let Some(rest) = line.trim_start().strip_prefix(&format!("{key}=")) else {
                return line.to_string();
            };
            let raw = rest.trim().trim_matches('"');
            match replace_port_in_url(raw, port) {
                Some(next) => {
                    changed = true;
                    format!("{key}={next}")
                }
                None => line.to_string(),
            }
        })
        .collect();
    if !changed {
        return;
    }
    let mut body = lines.join("\n");
    if text.ends_with('\n') {
        body.push('\n');
    }
    let _ = std::fs::write(path, body);
}

fn replace_port_in_url(url: &str, port: u16) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let hostport = authority.rsplit_once('@').map(|(_, host)| host).unwrap_or(authority);
    if hostport.contains(']') {
        return None;
    }
    let host = hostport.rsplit_once(':')?.0;
    let prefix_len = authority.len() - hostport.len();
    let prefix = &authority[..prefix_len];
    let path = if path.is_empty() && !rest.contains('/') {
        String::new()
    } else {
        format!("/{path}")
    };
    Some(format!("{scheme}://{prefix}{host}:{port}{path}"))
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
                "projects-mcp": {"image": "mcp", "ports": [{"published": "3201"}], "healthcheck": {"test": ["CMD", "fetch('/mcp')"]}},
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
                    id: "projects-mcp".into(),
                    url: "http://127.0.0.1:3201".into(),
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
                "projects-api": {"image": "projects", "restart": "unless-stopped", "profiles": ["extra"]},
                "hidden": {"image": "x", "restart": "unless-stopped", "labels": {"ira.catalog": "false"}},
                "named": {"image": "x", "ports": [{"target": 1}], "labels": ["ira.name=Bonito", "ira.required=true"]}
            }
        }"#;
        let entries = parse_catalog(raw).unwrap();
        let ids: Vec<_> = entries.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(ids, ["named", "postgres", "toolbox", "projects-api"]);
        assert!(
            entries
                .iter()
                .any(|entry| entry.id == "named" && entry.name == "Bonito" && entry.required)
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.id == "projects-api" && !entry.required)
        );
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
                .any(|s| s.id == "colibri" && !s.running)
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
                .any(|s| s.id == "colibri" && !s.healthy)
        );
    }

    #[test]
    fn splits_local_service_from_its_mcp() {
        let raw = r#"{
            "services": {
                "projects-api": {"image": "api", "restart": "unless-stopped", "labels": {"ira.peer": "projects-mcp"}},
                "projects-mcp": {"image": "mcp", "restart": "unless-stopped", "labels": {"ira.mcp": "http://127.0.0.1:8790/projects", "ira.peer": "projects-api"}}
            }
        }"#;
        let entries = parse_catalog(raw).unwrap();
        let api = entries.iter().find(|entry| entry.id == "projects-api").unwrap();
        let mcp = entries.iter().find(|entry| entry.id == "projects-mcp").unwrap();
        assert_eq!(api.kind, "service");
        assert_eq!(api.peer.as_deref(), Some("projects-mcp"));
        assert_eq!(mcp.kind, "mcp");
        assert_eq!(mcp.peer.as_deref(), Some("projects-api"));
    }

    #[test]
    fn rewrites_host_port_in_url() {
        assert_eq!(
            replace_port_in_url("postgres://ira:ira@127.0.0.1:5439/ira?sslmode=disable", 5440)
                .as_deref(),
            Some("postgres://ira:ira@127.0.0.1:5440/ira?sslmode=disable")
        );
        assert_eq!(
            replace_port_in_url("http://127.0.0.1:5000", 5001).as_deref(),
            Some("http://127.0.0.1:5001")
        );
    }

    fn unknown_service_is_rejected() {
        let catalog = builtin_catalog();
        assert!(known("postgres", &catalog).is_ok());
        assert!(known("colibri", &catalog).is_ok());
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
