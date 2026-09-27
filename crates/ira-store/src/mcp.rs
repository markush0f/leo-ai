use std::collections::HashMap;

use serde_json::Value;
use sqlx::{PgPool, Row};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTransport {
    Stdio,
    StreamableHttp,
}

impl McpTransport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::StreamableHttp => "streamable_http",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "stdio" => Some(Self::Stdio),
            "streamable_http" | "streamable-http" | "http" => Some(Self::StreamableHttp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub transport: McpTransport,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub url: Option<String>,
    pub enabled: bool,
}

pub type McpServer = McpServerConfig;

impl McpServerConfig {
    pub fn http(id: impl Into<String>, url: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            transport: McpTransport::StreamableHttp,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            headers: HashMap::new(),
            url: Some(url.into()),
            enabled: true,
        }
    }

    pub fn stdio(
        id: impl Into<String>,
        name: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Self {
        let id = id.into();
        let name = name.into();
        Self {
            id,
            name,
            transport: McpTransport::Stdio,
            command: Some(command.into()),
            args,
            env,
            headers: HashMap::new(),
            url: None,
            enabled: true,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() || self.name.trim().is_empty() {
            return Err("falta el nombre del MCP".into());
        }
        match self.transport {
            McpTransport::Stdio => {
                if self.command.as_deref().unwrap_or("").trim().is_empty() {
                    return Err("stdio requiere command".into());
                }
                if self.url.as_deref().is_some_and(|url| !url.trim().is_empty()) {
                    return Err("stdio no admite url".into());
                }
            }
            McpTransport::StreamableHttp => {
                if self.url.as_deref().unwrap_or("").trim().is_empty() {
                    return Err("http requiere url".into());
                }
                if self.command.as_deref().is_some_and(|cmd| !cmd.trim().is_empty()) {
                    return Err("http no admite command".into());
                }
            }
        }
        Ok(())
    }
}

const SELECT: &str = "SELECT id, name, transport, url, command, args, env, enabled FROM mcp_manager";

pub async fn list_mcp(pool: &PgPool) -> Result<Vec<McpServerConfig>, sqlx::Error> {
    let sql = format!("{SELECT} ORDER BY name");
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    rows.iter().map(row_config).collect()
}

pub async fn list_enabled_mcp(pool: &PgPool) -> Result<Vec<McpServerConfig>, sqlx::Error> {
    let sql = format!("{SELECT} WHERE enabled ORDER BY name");
    let rows = sqlx::query(&sql).fetch_all(pool).await?;
    rows.iter().map(row_config).collect()
}

pub async fn upsert_mcp(pool: &PgPool, server: &McpServerConfig) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO mcp_manager (id, name, transport, url, command, args, env, enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (id) DO UPDATE SET
            name = EXCLUDED.name,
            transport = EXCLUDED.transport,
            url = EXCLUDED.url,
            command = EXCLUDED.command,
            args = EXCLUDED.args,
            env = EXCLUDED.env,
            enabled = EXCLUDED.enabled,
            updated_at = now()",
    )
    .bind(&server.id)
    .bind(&server.name)
    .bind(server.transport.as_str())
    .bind(&server.url)
    .bind(&server.command)
    .bind(string_array(&server.args))
    .bind(string_map(&server.env))
    .bind(server.enabled)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_missing_mcp(
    pool: &PgPool,
    servers: &[McpServerConfig],
) -> Result<(), sqlx::Error> {
    for server in servers {
        let id = server.id.trim();
        let url = server.url.as_deref().unwrap_or("").trim();
        if id.is_empty() || url.is_empty() {
            continue;
        }
        sqlx::query(
            "INSERT INTO mcp_manager (id, name, transport, url, command, args, env, enabled)
             VALUES ($1, $2, 'streamable_http', $3, NULL, '[]'::jsonb, '{}'::jsonb, TRUE)
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(if server.name.trim().is_empty() {
            id
        } else {
            server.name.trim()
        })
        .bind(url)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn delete_mcp(pool: &PgPool, id_or_name: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM mcp_manager WHERE id = $1 OR name = $1")
        .bind(id_or_name)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

fn row_config(row: &sqlx::postgres::PgRow) -> Result<McpServerConfig, sqlx::Error> {
    let transport: String = row.try_get("transport")?;
    let transport = McpTransport::parse(&transport).ok_or_else(|| {
        sqlx::Error::Decode(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("transport MCP desconocido: {transport}"),
        )))
    })?;
    let args: Value = row.try_get("args")?;
    let env: Value = row.try_get("env")?;
    Ok(McpServerConfig {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        transport,
        command: row.try_get("command")?,
        args: json_strings(&args),
        env: json_map(&env),
        headers: HashMap::new(),
        url: row.try_get("url")?,
        enabled: row.try_get("enabled")?,
    })
}

fn string_array(args: &[String]) -> Value {
    Value::Array(args.iter().cloned().map(Value::String).collect())
}

fn string_map(env: &HashMap<String, String>) -> Value {
    Value::Object(
        env.iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect(),
    )
}

fn json_strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn json_map(value: &Value) -> HashMap<String, String> {
    value
        .as_object()
        .map(|map| {
            map.iter()
                .filter_map(|(key, item)| item.as_str().map(|value| (key.clone(), value.to_string())))
                .collect()
        })
        .unwrap_or_default()
}
