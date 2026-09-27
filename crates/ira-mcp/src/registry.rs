use std::time::Duration;

use ira_store::McpServerConfig;
use serde_json::Value;

use crate::error::Error;
use crate::vars::slug;

const OFFICIAL: &str = "https://registry.modelcontextprotocol.io";

pub struct McpRegistryClient {
    base: String,
    http: reqwest::Client,
}

impl McpRegistryClient {
    pub fn official() -> Self {
        Self::new(OFFICIAL)
    }

    pub fn new(base: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            base: base.into().trim_end_matches('/').to_string(),
            http,
        }
    }

    pub async fn find(&self, query: &str) -> Result<McpServerConfig, Error> {
        let query = query.trim();
        if query.is_empty() {
            return Err(Error::msg("falta el nombre"));
        }
        let mut last = Error::msg("registry no respondió");
        for path in ["/v0/servers", "/v0.1/servers"] {
            let response = self
                .http
                .get(format!("{}{path}", self.base))
                .query(&[("search", query), ("limit", "20")])
                .send()
                .await?;
            if response.status().as_u16() == 404 {
                continue;
            }
            if !response.status().is_success() {
                last = Error::msg(format!("registry http {}", response.status()));
                continue;
            }
            let body: Value = response.json().await?;
            return pick(&body, query).ok_or_else(|| Error::msg(format!("registry sin `{query}`")));
        }
        Err(last)
    }
}

pub fn pick(body: &Value, query: &str) -> Option<McpServerConfig> {
    let servers = body.get("servers").and_then(Value::as_array)?;
    let mut best: Option<(i32, McpServerConfig)> = None;
    for entry in servers {
        let server = entry.get("server").unwrap_or(entry);
        let Some(config) = server_config(server, query) else {
            continue;
        };
        let score = score(server, query);
        if best.as_ref().is_none_or(|(prev, _)| score > *prev) {
            best = Some((score, config));
        }
    }
    best.map(|(_, config)| config)
}

fn score(server: &Value, query: &str) -> i32 {
    let query = query.to_ascii_lowercase();
    let name = text(server, "name").to_ascii_lowercase();
    let ident = packages(server)
        .iter()
        .filter_map(|pkg| pkg.get("identifier").and_then(Value::as_str))
        .map(|value| value.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    if name == query || name.rsplit('/').next() == Some(query.as_str()) {
        100
    } else if name.ends_with(&format!("/{query}")) || ident.contains(&query) {
        80
    } else if text(server, "description")
        .to_ascii_lowercase()
        .contains(&query)
    {
        20
    } else {
        1
    }
}

fn server_config(server: &Value, query: &str) -> Option<McpServerConfig> {
    let raw_name = text(server, "name");
    let name = if raw_name.is_empty() {
        query.to_string()
    } else {
        raw_name
    };
    let id = slug(if name.contains('/') {
        name.rsplit('/').next().unwrap_or(&name)
    } else {
        &name
    });
    if let Some(config) = package_config(server, &id, &name) {
        return Some(config);
    }
    remote_config(server, &id, &name)
}

fn package_config(server: &Value, id: &str, name: &str) -> Option<McpServerConfig> {
    let packages = packages(server);
    let package = packages.iter().find(|pkg| kind(pkg) == "npm")
        .or_else(|| packages.iter().find(|pkg| kind(pkg) == "pypi"))
        .or_else(|| packages.iter().find(|pkg| matches!(kind(pkg).as_str(), "oci" | "docker")))?;
    let identifier = package.get("identifier").and_then(Value::as_str)?.trim();
    if identifier.is_empty() {
        return None;
    }
    let version = package
        .get("version")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && *value != "latest");
    let hint = package
        .get("runtimeHint")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let (command, mut args) = match kind(package).as_str() {
        "npm" => {
            let pkg = version
                .map(|version| format!("{identifier}@{version}"))
                .unwrap_or_else(|| identifier.to_string());
            ("npx".to_string(), vec!["-y".into(), pkg])
        }
        "pypi" => {
            let pkg = version
                .map(|version| format!("{identifier}=={version}"))
                .unwrap_or_else(|| identifier.to_string());
            ("uvx".to_string(), vec![pkg])
        }
        _ => (
            "docker".to_string(),
            vec!["run".into(), "-i".into(), "--rm".into(), identifier.into()],
        ),
    };
    if !hint.is_empty() && hint != command {
        if hint == "npx" && command == "npx" {
        } else if matches!(hint.as_str(), "npx" | "uvx" | "docker" | "podman") {
            args = match hint.as_str() {
                "npx" => vec!["-y".into(), args.last().cloned().unwrap_or_else(|| identifier.into())],
                "uvx" => vec![args.last().cloned().unwrap_or_else(|| identifier.into())],
                _ => args,
            };
            return Some(McpServerConfig::stdio(id, name, hint, args, env_of(package)));
        }
    }
    for arg in package_args(package) {
        args.push(arg);
    }
    Some(McpServerConfig::stdio(id, name, command, args, env_of(package)))
}

fn remote_config(server: &Value, id: &str, name: &str) -> Option<McpServerConfig> {
    let remotes = server.get("remotes").and_then(Value::as_array)?;
    let remote = remotes.iter().find(|remote| {
        matches!(
            remote.get("type").and_then(Value::as_str),
            Some("streamable-http" | "streamable_http" | "http" | "sse")
        )
    })?;
    let url = remote.get("url").and_then(Value::as_str)?.trim();
    if url.is_empty() {
        return None;
    }
    let mut config = McpServerConfig::http(id, url);
    config.name = name.to_string();
    Some(config)
}

fn packages(server: &Value) -> Vec<&Value> {
    server
        .get("packages")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn kind(package: &Value) -> String {
    package
        .get("registryType")
        .or_else(|| package.get("registry_type"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn env_of(package: &Value) -> std::collections::HashMap<String, String> {
    let mut env = std::collections::HashMap::new();
    let Some(items) = package
        .get("environmentVariables")
        .and_then(Value::as_array)
    else {
        return env;
    };
    for item in items {
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            continue;
        };
        if let Some(default) = item.get("default").and_then(Value::as_str) {
            env.insert(name.to_string(), default.to_string());
        } else if item
            .get("isRequired")
            .or_else(|| item.get("required"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            env.insert(name.to_string(), format!("${{SECRET:{name}}}"));
        }
    }
    env
}

fn package_args(package: &Value) -> Vec<String> {
    package
        .get("packageArguments")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("value").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ira_store::McpTransport;

    #[test]
    fn maps_npm_package_to_npx() {
        let body = serde_json::json!({
            "servers": [{
                "server": {
                    "name": "io.modelcontextprotocol/filesystem",
                    "packages": [{
                        "registryType": "npm",
                        "identifier": "@modelcontextprotocol/server-filesystem",
                        "version": "0.6.2",
                        "runtimeHint": "npx",
                        "transport": { "type": "stdio" }
                    }]
                }
            }]
        });
        let config = pick(&body, "filesystem").unwrap();
        assert_eq!(config.transport, McpTransport::Stdio);
        assert_eq!(config.command.as_deref(), Some("npx"));
        assert_eq!(
            config.args,
            vec!["-y", "@modelcontextprotocol/server-filesystem@0.6.2"]
        );
    }

    #[test]
    fn maps_remote_when_no_package() {
        let body = serde_json::json!({
            "servers": [{
                "name": "docs",
                "remotes": [{ "type": "streamable-http", "url": "https://example.com/mcp" }]
            }]
        });
        let config = pick(&body, "docs").unwrap();
        assert_eq!(config.transport, McpTransport::StreamableHttp);
        assert_eq!(config.url.as_deref(), Some("https://example.com/mcp"));
    }
}
