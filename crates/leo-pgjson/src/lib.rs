//! Exporta las tablas de una conexión PostgreSQL a JSON.
//!
//! La conexión indica una sola base. El volcado abre esa base en una
//! transacción de solo lectura, omite catálogos del sistema y particiones
//! hijas, y serializa cada fila con `to_jsonb`.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("conexión inválida: {0}")]
    InvalidConnection(String),
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error("no se pudo leer {schema}.{table}: {source}")]
    Table {
        schema: String,
        table: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("la tabla {schema}.{table} no devolvió un arreglo JSON")]
    NotAnArray { schema: String, table: String },
    #[error("no se pudo serializar el JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslMode {
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl SslMode {
    pub fn parse(value: &str) -> Result<Self, ExportError> {
        match value {
            "disable" => Ok(Self::Disable),
            "prefer" => Ok(Self::Prefer),
            "require" => Ok(Self::Require),
            "verify-ca" => Ok(Self::VerifyCa),
            "verify-full" => Ok(Self::VerifyFull),
            other => Err(ExportError::InvalidConnection(format!(
                "modo SSL inválido: {other}"
            ))),
        }
    }
}

impl From<SslMode> for PgSslMode {
    fn from(mode: SslMode) -> Self {
        match mode {
            SslMode::Disable => Self::Disable,
            SslMode::Prefer => Self::Prefer,
            SslMode::Require => Self::Require,
            SslMode::VerifyCa => Self::VerifyCa,
            SslMode::VerifyFull => Self::VerifyFull,
        }
    }
}

/// Parámetros de una base concreta. La contraseña no aparece en `Debug`.
pub struct DbConnection {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_mode: SslMode,
}

impl std::fmt::Debug for DbConnection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DbConnection")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("username", &self.username)
            .field("password", &"***")
            .field("ssl_mode", &self.ssl_mode)
            .finish()
    }
}

impl DbConnection {
    pub fn to_options(&self) -> Result<PgConnectOptions, ExportError> {
        if self.host.is_empty() || self.database.is_empty() || self.username.is_empty() {
            return Err(ExportError::InvalidConnection(
                "host, base de datos y usuario son obligatorios".into(),
            ));
        }
        if self.port == 0 {
            return Err(ExportError::InvalidConnection(
                "el puerto debe estar entre 1 y 65535".into(),
            ));
        }
        Ok(PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .database(&self.database)
            .username(&self.username)
            .password(&self.password)
            .ssl_mode(self.ssl_mode.into())
            .application_name("leo-pgjson"))
    }
}

pub fn options_from_url(url: &str) -> Result<PgConnectOptions, ExportError> {
    let options: PgConnectOptions = url
        .parse()
        .map_err(|err: sqlx::Error| ExportError::InvalidConnection(err.to_string()))?;
    if options.get_database().is_none() {
        return Err(ExportError::InvalidConnection(
            "la URL debe incluir el nombre de la base".into(),
        ));
    }
    Ok(options.application_name("leo-pgjson"))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportOptions {
    /// Vacío incluye todos los esquemas que no son del sistema.
    pub schemas: Vec<String>,
    /// Cada entrada es `tabla` o `esquema.tabla`. Vacío incluye todas.
    pub tables: Vec<String>,
    /// Máximo de filas por tabla. `None` lee la tabla completa.
    pub limit: Option<u64>,
    pub include_views: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dump {
    pub database: String,
    pub tables: Vec<TableJson>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableJson {
    pub schema: String,
    pub name: String,
    pub rows: Vec<serde_json::Value>,
}

pub fn include_table(schema: &str, table: &str, options: &ExportOptions) -> bool {
    if !options.schemas.is_empty() && !options.schemas.iter().any(|item| item == schema) {
        return false;
    }
    if options.tables.is_empty() {
        return true;
    }
    options
        .tables
        .iter()
        .any(|spec| table_spec_matches(spec, schema, table))
}

pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

pub fn to_json(dump: &Dump, pretty: bool) -> Result<String, ExportError> {
    if pretty {
        Ok(serde_json::to_string_pretty(dump)?)
    } else {
        Ok(serde_json::to_string(dump)?)
    }
}

pub async fn export(
    options: PgConnectOptions,
    request: &ExportOptions,
) -> Result<Dump, ExportError> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(15))
        .connect_with(options)
        .await?;
    let dump = export_with_pool(&pool, request).await;
    pool.close().await;
    dump
}

async fn export_with_pool(pool: &PgPool, request: &ExportOptions) -> Result<Dump, ExportError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await?;

    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&mut *tx)
        .await?;
    let found: Vec<(String, String)> = sqlx::query_as(&list_sql(request.include_views))
        .fetch_all(&mut *tx)
        .await?;

    let mut tables = Vec::new();
    for (schema, name) in found {
        if !include_table(&schema, &name, request) {
            continue;
        }
        let sql = table_sql(&schema, &name, request.limit);
        let value: serde_json::Value =
            sqlx::query_scalar(&sql)
                .fetch_one(&mut *tx)
                .await
                .map_err(|source| ExportError::Table {
                    schema: schema.clone(),
                    table: name.clone(),
                    source,
                })?;
        let serde_json::Value::Array(rows) = value else {
            return Err(ExportError::NotAnArray {
                schema,
                table: name,
            });
        };
        tables.push(TableJson { schema, name, rows });
    }

    tx.rollback().await?;
    Ok(Dump { database, tables })
}

fn list_sql(include_views: bool) -> String {
    let kinds = if include_views {
        "'r', 'p', 'f', 'v', 'm'"
    } else {
        "'r', 'p', 'f'"
    };
    format!(
        "SELECT n.nspname::text, c.relname::text
         FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE c.relkind IN ({kinds})
           AND n.nspname NOT IN ('pg_catalog', 'information_schema')
           AND n.nspname NOT LIKE 'pg\\_%'
           AND NOT c.relispartition
         ORDER BY 1, 2"
    )
}

fn table_sql(schema: &str, name: &str, limit: Option<u64>) -> String {
    let qualified = format!("{}.{}", quote_ident(schema), quote_ident(name));
    let limit_sql = limit
        .map(|count| format!(" LIMIT {count}"))
        .unwrap_or_default();
    format!(
        "SELECT COALESCE(json_agg(to_jsonb(t)), '[]'::json) \
         FROM (SELECT * FROM {qualified}{limit_sql}) t"
    )
}

fn table_spec_matches(spec: &str, schema: &str, table: &str) -> bool {
    match spec.split_once('.') {
        Some((wanted_schema, wanted_table)) => wanted_schema == schema && wanted_table == table,
        None => spec == table,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_identifiers() {
        assert_eq!(quote_ident("public"), "\"public\"");
        assert_eq!(quote_ident("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn table_query_limits_and_quotes() {
        let sql = table_sql("public", "a\"b", Some(10));
        assert!(sql.contains("\"public\".\"a\"\"b\""));
        assert!(sql.contains("LIMIT 10"));
        assert!(!sql.contains("LIMIT 10 LIMIT"));
    }

    #[test]
    fn filters_schema_and_table() {
        let options = ExportOptions {
            schemas: vec!["public".into()],
            tables: vec!["messages".into(), "secret.tokens".into()],
            ..ExportOptions::default()
        };
        assert!(include_table("public", "messages", &options));
        assert!(!include_table("secret", "tokens", &options));
        assert!(!include_table("public", "conversations", &options));

        let by_name = ExportOptions {
            tables: vec!["conversations".into()],
            ..ExportOptions::default()
        };
        assert!(include_table("audit", "conversations", &by_name));
    }

    #[test]
    fn rejects_incomplete_connection_and_bad_ssl() {
        let connection = DbConnection {
            host: String::new(),
            port: 5432,
            database: "leo".into(),
            username: "leo".into(),
            password: String::new(),
            ssl_mode: SslMode::Disable,
        };
        assert!(connection.to_options().is_err());
        assert!(SslMode::parse("allow").is_err());
        assert_eq!(SslMode::parse("verify-full").unwrap(), SslMode::VerifyFull);
    }

    #[test]
    fn serializes_a_dump() {
        let dump = Dump {
            database: "leo".into(),
            tables: vec![TableJson {
                schema: "public".into(),
                name: "events".into(),
                rows: vec![serde_json::json!({"id": 7, "title": "hola"})],
            }],
        };
        let json = to_json(&dump, false).unwrap();
        let parsed: Dump = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, dump);
    }

    #[test]
    fn url_requires_a_database_name() {
        let error = options_from_url("postgres://leo:leo@127.0.0.1:5439").unwrap_err();
        assert!(error.to_string().contains("nombre de la base"));
    }
}
