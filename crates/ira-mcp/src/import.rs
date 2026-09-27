use std::collections::HashMap;
use std::path::Path;

use ira_store::{McpServerConfig, McpTransport};
use serde_json::Value;

use crate::error::Error;
use crate::vars::slug;

pub fn parse_claude(body: &str) -> Result<Vec<McpServerConfig>, Error> {
    let value: Value = serde_json::from_str(body)?;
    let servers = value
        .get("mcpServers")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::msg("falta mcpServers"))?;
    let mut out = Vec::new();
    for (name, spec) in servers {
        out.push(one(name, spec)?);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub async fn load_file(path: &Path) -> Result<Vec<McpServerConfig>, Error> {
    let body = tokio::fs::read_to_string(path)
        .await
        .map_err(|err| Error::msg(format!("no se pudo leer {}: {err}", path.display())))?;
    parse_claude(&body)
}

fn one(name: &str, spec: &Value) -> Result<McpServerConfig, Error> {
    let id = slug(name);
    if id.is_empty() {
        return Err(Error::msg("nombre MCP vacío"));
    }
    let env = env_map(spec.get("env"));
    let args = string_list(spec.get("args"));
    let url = spec
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let command = spec
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let kind = spec
        .get("type")
        .or_else(|| spec.get("transport"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let http = url.is_some()
        || matches!(
            kind,
            "http" | "sse" | "streamable_http" | "streamable-http"
        );
    if http {
        let url = url.ok_or_else(|| Error::msg(format!("{name}: http sin url")))?;
        let mut config = McpServerConfig::http(id, url);
        config.name = name.to_string();
        config.env = env;
        return Ok(config);
    }
    let command = command.ok_or_else(|| Error::msg(format!("{name}: falta command o url")))?;
    let mut args = args;
    for arg in &mut args {
        if arg == "." {
            *arg = "${IRA_WORKSPACE}".into();
        }
    }
    let mut config = McpServerConfig::stdio(id, name, command, args, env);
    if kind == "stdio" || kind.is_empty() {
        config.transport = McpTransport::Stdio;
    }
    config.validate().map_err(Error::msg)?;
    Ok(config)
}

fn env_map(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(key, item)| item.as_str().map(|value| (key.clone(), value.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn from_add(name: &str, url: Option<&str>, command: &[String]) -> Result<McpServerConfig, Error> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::msg("falta el nombre"));
    }
    let id = slug(name);
    let url = url.map(str::trim).filter(|value| !value.is_empty());
    match (url, command.is_empty()) {
        (Some(url), true) => {
            let mut config = McpServerConfig::http(id, url);
            config.name = name.to_string();
            Ok(config)
        }
        (None, false) => {
            let program = command[0].trim();
            if program.is_empty() {
                return Err(Error::msg("command vacío"));
            }
            let mut args = command[1..].to_vec();
            for arg in &mut args {
                if arg == "." {
                    *arg = "${IRA_WORKSPACE}".into();
                }
            }
            Ok(McpServerConfig::stdio(
                id,
                name,
                program,
                args,
                HashMap::new(),
            ))
        }
        (Some(_), false) => Err(Error::msg("usa --url o un comando, no ambos")),
        (None, true) => Err(Error::msg("falta --url o el comando tras --")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ira_store::McpTransport;

    #[test]
    fn imports_claude_stdio_and_http() {
        let configs = parse_claude(
            r#"{
                "mcpServers": {
                    "example": { "command": "node", "args": ["server.js", "."], "env": { "A": "1" } },
                    "docs": { "url": "https://example.com/mcp" }
                }
            }"#,
        )
        .unwrap();
        assert_eq!(configs.len(), 2);
        let example = configs.iter().find(|cfg| cfg.name == "example").unwrap();
        assert_eq!(example.transport, McpTransport::Stdio);
        assert_eq!(example.command.as_deref(), Some("node"));
        assert_eq!(example.args[1], "${IRA_WORKSPACE}");
        assert_eq!(example.env.get("A").map(String::as_str), Some("1"));
        let docs = configs.iter().find(|cfg| cfg.name == "docs").unwrap();
        assert_eq!(docs.transport, McpTransport::StreamableHttp);
        assert_eq!(docs.url.as_deref(), Some("https://example.com/mcp"));
    }

    #[test]
    fn add_stdio_rewrites_dot() {
        let config = from_add(
            "filesystem",
            None,
            &[
                "npx".into(),
                "-y".into(),
                "@modelcontextprotocol/server-filesystem".into(),
                ".".into(),
            ],
        )
        .unwrap();
        assert_eq!(config.transport, McpTransport::Stdio);
        assert_eq!(config.command.as_deref(), Some("npx"));
        assert_eq!(config.args.last().map(String::as_str), Some("${IRA_WORKSPACE}"));
    }
}
