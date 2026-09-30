use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use ira_store::{McpServerConfig, McpTransport};
use serde_json::{Map, Value, json};

use crate::error::Error;
use crate::vars::{ira_home, slug};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    Project,
}

pub fn global_path() -> PathBuf {
    ira_home().join("ira.json")
}

pub fn project_file() -> Option<PathBuf> {
    find_up("ira.json")
}

pub fn project_path_for_write() -> PathBuf {
    project_file().unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("ira.json")
    })
}

pub fn path_for(scope: Scope) -> PathBuf {
    match scope {
        Scope::Global => global_path(),
        Scope::Project => project_path_for_write(),
    }
}

pub fn enabled_servers() -> Vec<McpServerConfig> {
    match load_enabled() {
        Ok(servers) => servers,
        Err(err) => {
            tracing::warn!(%err, "mcp config");
            Vec::new()
        }
    }
}

pub fn load_enabled() -> Result<Vec<McpServerConfig>, Error> {
    Ok(load_all()?
        .into_iter()
        .filter(|server| server.enabled)
        .collect())
}

pub fn load_all() -> Result<Vec<McpServerConfig>, Error> {
    let mut servers = read_servers(&global_path())?;
    if let Some(path) = project_file() {
        if path != global_path() {
            for server in read_servers(&path)? {
                if let Some(slot) = servers.iter_mut().find(|item| item.id == server.id) {
                    *slot = server;
                } else {
                    servers.push(server);
                }
            }
        }
    }
    servers.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(servers)
}

pub fn upsert(scope: Scope, config: &McpServerConfig) -> Result<PathBuf, Error> {
    config.validate().map_err(Error::msg)?;
    let path = path_for(scope);
    let mut servers = read_servers(&path)?;
    if let Some(slot) = servers.iter_mut().find(|item| item.id == config.id) {
        *slot = config.clone();
    } else {
        servers.push(config.clone());
    }
    write_servers(&path, &servers)?;
    Ok(path)
}

pub fn remove(name: &str) -> Result<Vec<PathBuf>, Error> {
    let key = name.trim();
    let id = slug(key);
    let mut touched = Vec::new();
    for path in [global_path(), project_file().unwrap_or_default()] {
        if path.as_os_str().is_empty() || !path.is_file() {
            continue;
        }
        let mut servers = read_servers(&path)?;
        let before = servers.len();
        servers.retain(|server| server.id != id && server.name != key);
        if servers.len() != before {
            write_servers(&path, &servers)?;
            touched.push(path);
        }
    }
    if touched.is_empty() {
        return Err(Error::msg(format!("no existe {key}")));
    }
    Ok(touched)
}

pub fn remove_global(name: &str) -> Result<(), Error> {
    let path = global_path();
    let mut servers = read_servers(&path)?;
    let before = servers.len();
    servers.retain(|server| server.id != name);
    if servers.len() == before {
        return Err(Error::msg(format!(
            "no existe {name} en configuración global"
        )));
    }
    write_servers(&path, &servers)
}

pub fn read_servers(path: &Path) -> Result<Vec<McpServerConfig>, Error> {
    let doc = read_document(path)?;
    let Some(mcp) = doc.get("mcp") else {
        return Ok(Vec::new());
    };
    let map = mcp
        .as_object()
        .ok_or_else(|| Error::msg(format!("{}: mcp debe ser un objeto", path.display())))?;
    let mut servers = Vec::new();
    for (name, spec) in map {
        servers.push(parse_server(name, spec)?);
    }
    servers.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(servers)
}

fn write_servers(path: &Path, servers: &[McpServerConfig]) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut doc = read_document(path)?;
    let mut mcp = Map::new();
    let mut ordered = servers.to_vec();
    ordered.sort_by(|left, right| left.name.cmp(&right.name));
    for server in ordered {
        mcp.insert(server.name.clone(), server_json(&server));
    }
    doc.insert("mcp".into(), Value::Object(mcp));
    let body = serde_json::to_string_pretty(&Value::Object(doc))? + "\n";
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, body)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn read_document(path: &Path) -> Result<Map<String, Value>, Error> {
    if !path.is_file() {
        return Ok(Map::new());
    }
    let text = fs::read_to_string(path)
        .map_err(|err| Error::msg(format!("no se pudo leer {}: {err}", path.display())))?;
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str(&text)? {
        Value::Object(map) => Ok(map),
        _ => Err(Error::msg(format!("{} debe ser un objeto", path.display()))),
    }
}

fn server_json(config: &McpServerConfig) -> Value {
    match config.transport {
        McpTransport::Stdio => {
            let mut command = vec![config.command.clone().unwrap_or_default()];
            command.extend(config.args.iter().cloned());
            let mut value = json!({
                "type": "local",
                "command": command,
                "enabled": config.enabled,
            });
            if !config.env.is_empty() {
                value["environment"] = map_json(&config.env);
            }
            value
        }
        McpTransport::StreamableHttp => {
            let mut value = json!({
                "type": "remote",
                "url": config.url.clone().unwrap_or_default(),
                "enabled": config.enabled,
            });
            if !config.headers.is_empty() {
                value["headers"] = map_json(&config.headers);
            }
            value
        }
    }
}

fn parse_server(name: &str, spec: &Value) -> Result<McpServerConfig, Error> {
    let kind = spec
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let enabled = spec.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let remote = matches!(
        kind.as_str(),
        "remote" | "http" | "streamable-http" | "streamable_http"
    ) || (kind.is_empty() && spec.get("url").is_some());
    if remote {
        let url = spec
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .ok_or_else(|| Error::msg(format!("{name}: remote sin url")))?;
        let mut config = McpServerConfig::http(slug(name), url);
        config.name = name.to_string();
        config.enabled = enabled;
        config.headers = string_map(spec.get("headers"));
        return config.validate().map_err(Error::msg).map(|()| config);
    }
    let command = command_list(spec.get("command"))
        .ok_or_else(|| Error::msg(format!("{name}: local sin command")))?;
    let program = command
        .first()
        .map(|item| item.trim().to_string())
        .unwrap_or_default();
    if program.is_empty() {
        return Err(Error::msg(format!("{name}: command vacío")));
    }
    let mut config = McpServerConfig::stdio(
        slug(name),
        name,
        program,
        command.into_iter().skip(1).collect(),
        string_map(spec.get("environment").or_else(|| spec.get("env"))),
    );
    config.enabled = enabled;
    config.validate().map_err(Error::msg)?;
    Ok(config)
}

fn command_list(value: Option<&Value>) -> Option<Vec<String>> {
    match value? {
        Value::String(text) if !text.trim().is_empty() => Some(vec![text.clone()]),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect();
            (!parts.is_empty()).then_some(parts)
        }
        _ => None,
    }
}

fn string_map(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(key, item)| {
                    item.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn map_json(map: &HashMap<String, String>) -> Value {
    Value::Object(
        map.iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect(),
    )
}

fn find_up(name: &str) -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if dir.join(".git").exists() {
            return None;
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_local_and_remote() {
        let dir = std::env::temp_dir().join(format!("ira-mcp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ira.json");
        let local = McpServerConfig::stdio(
            "filesystem",
            "filesystem",
            "npx",
            vec![
                "-y".into(),
                "@modelcontextprotocol/server-filesystem".into(),
            ],
            HashMap::from([("FOO".into(), "1".into())]),
        );
        let mut remote = McpServerConfig::http("docs", "https://example.com/mcp");
        remote.name = "docs".into();
        remote.headers = HashMap::from([("Authorization".into(), "Bearer {env:TOKEN}".into())]);
        write_servers(&path, &[local, remote]).unwrap();
        let servers = read_servers(&path).unwrap();
        assert_eq!(servers.len(), 2);
        let filesystem = servers
            .iter()
            .find(|server| server.name == "filesystem")
            .unwrap();
        assert_eq!(filesystem.transport, McpTransport::Stdio);
        assert_eq!(filesystem.command.as_deref(), Some("npx"));
        assert_eq!(filesystem.env.get("FOO").map(String::as_str), Some("1"));
        let docs = servers.iter().find(|server| server.name == "docs").unwrap();
        assert_eq!(docs.transport, McpTransport::StreamableHttp);
        assert_eq!(
            docs.headers.get("Authorization").map(String::as_str),
            Some("Bearer {env:TOKEN}")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_key_overrides_global() {
        let root = std::env::temp_dir().join(format!("ira-mcp-merge-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let global = root.join("global.json");
        let project = root.join("project.json");
        let mut base = McpServerConfig::http("docs", "https://old.example/mcp");
        base.name = "docs".into();
        let mut over = McpServerConfig::http("docs", "https://new.example/mcp");
        over.name = "docs".into();
        over.enabled = false;
        write_servers(&global, &[base]).unwrap();
        write_servers(&project, &[over]).unwrap();
        let mut servers = read_servers(&global).unwrap();
        for server in read_servers(&project).unwrap() {
            if let Some(slot) = servers.iter_mut().find(|item| item.id == server.id) {
                *slot = server;
            } else {
                servers.push(server);
            }
        }
        assert_eq!(servers[0].url.as_deref(), Some("https://new.example/mcp"));
        assert!(!servers[0].enabled);
        let _ = fs::remove_dir_all(&root);
    }
}
