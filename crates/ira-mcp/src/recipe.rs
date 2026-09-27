use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use ira_store::McpServerConfig;
use serde_json::Value;
use tokio::process::Command;

use crate::error::{Error, clip};
use crate::vars::{VarCtx, resolve_value, slug};

const VERITAS: &str = include_str!("../../../resources/mcp-recipes/veritas-kanban.json");

#[derive(Debug, Clone)]
pub struct Recipe {
    pub name: String,
    pub repository: String,
    pub git_ref: Option<String>,
    pub install: Vec<String>,
    pub build: Vec<String>,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub env_requirements: Vec<String>,
}

pub fn find(name: &str) -> Option<Recipe> {
    let key = name.trim().to_ascii_lowercase();
    let builtin = match key.as_str() {
        "veritas-kanban" | "veritas" => Some(VERITAS),
        _ => None,
    };
    if let Some(body) = builtin
        && let Ok(recipe) = parse(body)
    {
        return Some(recipe);
    }
    let path = recipes_dir().join(format!("{key}.json"));
    let body = std::fs::read_to_string(path).ok()?;
    parse(&body).ok()
}

pub fn recipes_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("IRA_MCP_RECIPES")
        && !dir.trim().is_empty()
    {
        return PathBuf::from(dir);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for candidate in [
        cwd.join("resources/mcp-recipes"),
        cwd.join("../resources/mcp-recipes"),
    ] {
        if candidate.is_dir() {
            return candidate;
        }
    }
    cwd.join("resources/mcp-recipes")
}

pub fn parse(body: &str) -> Result<Recipe, Error> {
    let value: Value = serde_json::from_str(body)?;
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::msg("receta sin name"))?
        .to_string();
    let command = value
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::msg("receta sin command"))?
        .to_string();
    Ok(Recipe {
        name,
        repository: value
            .get("repository")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        git_ref: value
            .get("ref")
            .and_then(Value::as_str)
            .map(str::to_string),
        install: strings(value.get("install")),
        build: strings(value.get("build")),
        command,
        args: strings(value.get("args")),
        env: string_map(value.get("env")),
        env_requirements: strings(value.get("env_requirements")),
    })
}

pub async fn execute(recipe: &Recipe) -> Result<McpServerConfig, Error> {
    let id = slug(&recipe.name);
    let dest = VarCtx::for_server(&id).install_dir;
    if !recipe.repository.trim().is_empty() {
        checkout(recipe, &dest).await?;
    } else if !dest.exists() {
        tokio::fs::create_dir_all(&dest).await?;
    }
    for command in recipe.install.iter().chain(recipe.build.iter()) {
        tracing::info!(dir = %dest.display(), %command, "mcp recipe");
        run(&dest, command).await?;
    }
    let ctx = VarCtx {
        install_dir: dest,
        ..VarCtx::for_server(&id)
    };
    for key in &recipe.env_requirements {
        let value = recipe
            .env
            .get(key)
            .map(|value| resolve_value(value, &ctx))
            .transpose()?
            .or_else(|| std::env::var(key).ok());
        if value.as_deref().unwrap_or("").trim().is_empty() {
            return Err(Error::msg(format!("la receta requiere `{key}`")));
        }
    }
    let config = McpServerConfig::stdio(
        id,
        recipe.name.clone(),
        recipe.command.clone(),
        recipe.args.clone(),
        recipe.env.clone(),
    );
    config.validate().map_err(Error::msg)?;
    Ok(config)
}

async fn checkout(recipe: &Recipe, dest: &Path) -> Result<(), Error> {
    if dest.join(".git").is_dir() {
        tracing::info!(dir = %dest.display(), "mcp recipe ya clonada");
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tracing::info!(repo = %recipe.repository, dir = %dest.display(), "clonando MCP");
    let mut cmd = Command::new("git");
    cmd.arg("clone").arg("--depth").arg("1");
    if let Some(git_ref) = &recipe.git_ref {
        cmd.arg("--branch").arg(git_ref);
    }
    cmd.arg(&recipe.repository).arg(dest);
    let status = cmd
        .status()
        .await
        .map_err(|err| Error::msg(format!("git no disponible: {err}")))?;
    if !status.success() {
        return Err(Error::msg(format!(
            "git clone falló ({})",
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

async fn run(dir: &Path, line: &str) -> Result<(), Error> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(line)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .await?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(Error::msg(format!(
        "{line}: {}",
        clip(&format!("{stderr}{stdout}"))
    )))
}

fn strings(value: Option<&Value>) -> Vec<String> {
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

fn string_map(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(key, item)| item.as_str().map(|value| (key.clone(), value.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_veritas_recipe() {
        let recipe = find("veritas-kanban").unwrap();
        assert_eq!(recipe.command, "node");
        assert!(recipe.args[0].contains("${INSTALL_DIR}"));
        assert!(!recipe.repository.is_empty());
    }
}
