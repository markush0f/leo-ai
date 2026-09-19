use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use leo_store::{self as db, DatabaseCipher, DatabaseConnectionRow};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Serialize)]
struct Source<'a> {
    kind: &'static str,
    name: String,
    #[serde(rename = "type")]
    source_type: &'static str,
    host: &'a str,
    port: String,
    database: &'a str,
    user: &'a str,
    password: &'a str,
    #[serde(rename = "queryParams")]
    query_params: BTreeMap<&'static str, &'a str>,
    #[serde(rename = "connectTimeout")]
    connect_timeout: u8,
}

#[derive(Serialize)]
struct Tool<'a> {
    kind: &'static str,
    name: String,
    #[serde(rename = "type")]
    tool_type: &'static str,
    source: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    statement: Option<&'a str>,
}

#[derive(Serialize)]
struct Group {
    kind: &'static str,
    name: String,
    description: String,
    tools: Vec<String>,
}

pub async fn sync(pool: &PgPool, cipher: &DatabaseCipher) -> Result<(), String> {
    let dir = prepare().await?;

    let rows = db::list_database_connections(pool)
        .await
        .map_err(|err| err.to_string())?;
    let active: Vec<&DatabaseConnectionRow> = rows
        .iter()
        .filter(|row| row.enabled && row.last_test_ok == Some(true))
        .collect();
    let mut rendered = Vec::new();
    for row in active {
        let password = match db::database_password(pool, cipher, row.id).await {
            Ok(password) => password,
            Err(error) => {
                reset().await?;
                return Err(error.to_string());
            }
        };
        if password.is_empty() {
            reset().await?;
            return Err(format!("{} no tiene contraseña", row.name));
        }
        let name = format!("leo-{}.yaml", row.id.simple());
        let contents = match render(row, &password) {
            Ok(contents) => contents,
            Err(error) => {
                reset().await?;
                return Err(error);
            }
        };
        rendered.push((name, contents));
    }
    let mut expected: Vec<String> = rendered.iter().map(|(name, _)| name.clone()).collect();
    expected.push("leo-base.yaml".into());
    remove_stale(&dir, &expected).await?;
    for (name, contents) in rendered {
        write_private(&dir.join(name), &contents).await?;
    }
    Ok(())
}

pub async fn prepare() -> Result<PathBuf, String> {
    let dir = config_dir()?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|err| format!("no se pudo crear configuración Toolbox: {err}"))?;
    write_private(
        &dir.join("leo-base.yaml"),
        "kind: group\nname: leo_databases\ndescription: Bases de datos habilitadas en Leo.\ntools: []\n",
    )
    .await?;
    Ok(dir)
}

pub async fn reset() -> Result<(), String> {
    let dir = prepare().await?;
    remove_stale(&dir, &["leo-base.yaml".into()]).await
}

fn render(row: &DatabaseConnectionRow, password: &str) -> Result<String, String> {
    let prefix = managed_prefix(row.id);
    let source_name = format!("{prefix}_source");
    let specs = [
        (
            "query",
            "postgres-execute-sql",
            format!(
                "Ejecuta SQL en {}. La cuenta PostgreSQL debe tener permisos de solo lectura.",
                row.name
            ),
        ),
        (
            "list_tables",
            "postgres-list-tables",
            format!("Lista tablas y columnas de {}.", row.name),
        ),
        (
            "list_schemas",
            "postgres-list-schemas",
            format!("Lista esquemas de {}.", row.name),
        ),
        (
            "list_views",
            "postgres-list-views",
            format!("Lista vistas de {}.", row.name),
        ),
        (
            "overview",
            "postgres-database-overview",
            format!("Resume estructura y tamaño de {}.", row.name),
        ),
    ];
    let tool_names: Vec<String> = specs
        .iter()
        .map(|(suffix, _, _)| format!("{prefix}_{suffix}"))
        .collect();
    let mut query_params = BTreeMap::new();
    query_params.insert("sslmode", row.ssl_mode.as_str());
    query_params.insert("application_name", "leo-ai");
    query_params.insert("options", "-c default_transaction_read_only=on");
    let source = Source {
        kind: "source",
        name: source_name.clone(),
        source_type: "postgres",
        host: &row.host,
        port: row.port.to_string(),
        database: &row.database,
        user: &row.username,
        password,
        query_params,
        connect_timeout: 10,
    };
    let group = Group {
        kind: "group",
        name: prefix.clone(),
        description: format!("Herramientas PostgreSQL para {}.", row.name),
        tools: tool_names.clone(),
    };
    let mut documents = vec![yaml(&source)?];
    for ((_, tool_type, description), name) in specs.iter().zip(tool_names) {
        documents.push(yaml(&Tool {
            kind: "tool",
            name,
            tool_type,
            source: source_name.clone(),
            description: description.clone(),
            statement: None,
        })?);
    }
    documents.push(yaml(&group)?);
    Ok(documents.join("---\n"))
}

pub fn managed_query_name(id: uuid::Uuid) -> String {
    format!("{}_query", managed_prefix(id))
}

fn managed_prefix(id: uuid::Uuid) -> String {
    format!("db_{}", id.simple())
}

fn yaml(value: &impl Serialize) -> Result<String, String> {
    serde_yaml::to_string(value).map_err(|err| format!("configuración Toolbox inválida: {err}"))
}

fn config_dir() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("LEO_TOOLBOX_CONFIG_DIR") {
        return Ok(PathBuf::from(path));
    }
    let root = super::host::workspace_root()
        .ok_or_else(|| "no encuentro raíz para configuración Toolbox".to_string())?;
    Ok(root.join(".leo/toolbox"))
}

async fn write_private(path: &Path, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, contents)
        .await
        .map_err(|err| format!("no se pudo escribir configuración Toolbox: {err}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(|err| format!("no se pudo proteger configuración Toolbox: {err}"))?;
    }
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|err| format!("no se pudo activar configuración Toolbox: {err}"))
}

async fn remove_stale(dir: &Path, expected: &[String]) -> Result<(), String> {
    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|err| format!("no se pudo leer configuración Toolbox: {err}"))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|err| format!("no se pudo leer configuración Toolbox: {err}"))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("leo-") && name.ends_with(".yaml") && !expected.contains(&name) {
            tokio::fs::remove_file(entry.path())
                .await
                .map_err(|err| format!("no se pudo retirar configuración Toolbox: {err}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn yaml_quotes_credentials_and_namespaces_tools() {
        let id = Uuid::from_u128(1);
        let output = render(
            &DatabaseConnectionRow {
                id,
                name: "Ventas".into(),
                host: "db.internal".into(),
                port: 5432,
                database: "sales".into(),
                username: "reader".into(),
                ssl_mode: "require".into(),
                enabled: true,
                password_set: true,
                last_test_ok: None,
                last_test_error: None,
                last_tested_at: None,
            },
            "a: #secret",
        )
        .unwrap();
        assert!(output.contains("db_00000000000000000000000000000001_query"));
        assert!(output.contains("password: 'a: #secret'"));
    }
}
