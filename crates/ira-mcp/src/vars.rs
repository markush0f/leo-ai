use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ira_store::McpServerConfig;

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct VarCtx {
    pub ira_home: PathBuf,
    pub workspace: PathBuf,
    pub install_dir: PathBuf,
}

impl VarCtx {
    pub fn for_server(id: &str) -> Self {
        let ira_home = ira_home();
        let workspace = std::env::var("IRA_WORKSPACE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            install_dir: ira_home.join("mcp").join(id),
            ira_home,
            workspace,
        }
    }
}

pub fn ira_home() -> PathBuf {
    ira_store::ira_home()
}

pub fn resolve_config(config: &McpServerConfig) -> Result<McpServerConfig, Error> {
    let ctx = VarCtx::for_server(&config.id);
    let mut resolved = config.clone();
    if let Some(command) = resolved.command.as_mut() {
        *command = resolve_value(command, &ctx)?;
    }
    for arg in &mut resolved.args {
        *arg = resolve_value(arg, &ctx)?;
    }
    if let Some(url) = resolved.url.as_mut() {
        *url = resolve_value(url, &ctx)?;
    }
    let mut env = HashMap::new();
    for (key, value) in &resolved.env {
        env.insert(key.clone(), resolve_value(value, &ctx)?);
    }
    resolved.env = env;
    let mut headers = HashMap::new();
    for (key, value) in &resolved.headers {
        headers.insert(key.clone(), resolve_value(value, &ctx)?);
    }
    resolved.headers = headers;
    Ok(resolved)
}

pub fn resolve_value(raw: &str, ctx: &VarCtx) -> Result<String, Error> {
    let raw = expand_env_braces(raw);
    let mut out = String::new();
    let mut rest = raw.as_str();
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            return Err(Error::msg(format!("variable sin cerrar en `{raw}`")));
        };
        let key = &after[..end];
        out.push_str(&lookup(key, ctx)?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn expand_env_braces(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(start) = rest.find("{env:") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 5..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        out.push_str(&std::env::var(&after[..end]).unwrap_or_default());
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn lookup(key: &str, ctx: &VarCtx) -> Result<String, Error> {
    if let Some(name) = key.strip_prefix("SECRET:") {
        return std::env::var(name).map_err(|_| {
            Error::msg(format!(
                "falta el secreto `{name}` (tabla secrets o entorno)"
            ))
        });
    }
    let path: &Path = match key {
        "IRA_HOME" => &ctx.ira_home,
        "IRA_WORKSPACE" => &ctx.workspace,
        "INSTALL_DIR" => &ctx.install_dir,
        other => {
            return Err(Error::msg(format!(
                "variable desconocida `${{{other}}}`"
            )));
        }
    };
    Ok(path.display().to_string())
}

pub fn slug(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' | '_' | '-' | '.' => ch,
            '/' | ' ' => '-',
            _ => '-',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_known_vars_and_dot_placeholder() {
        let ctx = VarCtx {
            ira_home: PathBuf::from("/home/ira"),
            workspace: PathBuf::from("/work"),
            install_dir: PathBuf::from("/home/ira/mcp/demo"),
        };
        assert_eq!(
            resolve_value("${INSTALL_DIR}/mcp/dist/index.js", &ctx).unwrap(),
            "/home/ira/mcp/demo/mcp/dist/index.js"
        );
        assert_eq!(resolve_value("${IRA_WORKSPACE}", &ctx).unwrap(), "/work");
    }

    #[test]
    fn rejects_unknown_var() {
        let ctx = VarCtx::for_server("demo");
        assert!(resolve_value("${NOPE}", &ctx).is_err());
    }

    #[test]
    fn slug_flattens_registry_names() {
        assert_eq!(slug("io.github.foo/bar"), "io.github.foo-bar");
    }
}
