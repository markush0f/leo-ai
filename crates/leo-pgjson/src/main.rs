//! Vuelca a JSON las tablas de una conexión PostgreSQL.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use leo_pgjson::{DbConnection, ExportOptions, SslMode, export, options_from_url, to_json};

#[derive(Parser)]
#[command(
    name = "leo-pgjson",
    about = "Exporta las tablas de una conexión PostgreSQL a JSON"
)]
struct Cli {
    /// URL `postgres://usuario:clave@host:puerto/base`.
    #[arg(long, env = "LEO_PGJSON_URL")]
    url: Option<String>,
    #[arg(long)]
    host: Option<String>,
    #[arg(long, default_value_t = 5432)]
    port: u16,
    #[arg(long)]
    database: Option<String>,
    #[arg(long)]
    username: Option<String>,
    #[arg(long, env = "PGPASSWORD")]
    password: Option<String>,
    /// `disable`, `prefer`, `require`, `verify-ca` o `verify-full`.
    #[arg(long, default_value = "disable")]
    ssl_mode: String,
    /// Esquema a incluir. Repetible.
    #[arg(long = "schema")]
    schemas: Vec<String>,
    /// Tabla `nombre` o `esquema.nombre`. Repetible.
    #[arg(long = "table")]
    tables: Vec<String>,
    /// Máximo de filas por tabla.
    #[arg(long)]
    limit: Option<u64>,
    /// Incluye vistas y vistas materializadas.
    #[arg(long)]
    views: bool,
    /// Incluye solo esquema, claves y relaciones; no lee filas.
    #[arg(long)]
    schema_only: bool,
    /// Archivo de salida. Sin este argumento escribe en stdout.
    #[arg(long)]
    out: Option<PathBuf>,
    /// JSON en una sola línea.
    #[arg(long)]
    compact: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let options = match (cli.url, cli.host) {
        (Some(_), Some(_)) => {
            return Err("usa --url o los campos sueltos, no ambos".into());
        }
        (Some(url), None) => options_from_url(&url)?,
        (None, Some(host)) => {
            let database = cli
                .database
                .filter(|value| !value.is_empty())
                .ok_or("falta --database")?;
            let username = cli
                .username
                .filter(|value| !value.is_empty())
                .ok_or("falta --username")?;
            DbConnection {
                host,
                port: cli.port,
                database,
                username,
                password: cli.password.unwrap_or_default(),
                ssl_mode: SslMode::parse(&cli.ssl_mode)?,
            }
            .to_options()?
        }
        (None, None) => {
            return Err("indica --url o --host, --database y --username".into());
        }
    };
    let request = ExportOptions {
        schemas: cli.schemas,
        tables: cli.tables,
        limit: cli.limit,
        include_views: cli.views,
        schema_only: cli.schema_only,
    };
    let dump = export(options, &request).await?;
    let json = to_json(&dump, !cli.compact)?;
    match cli.out {
        Some(path) => std::fs::write(&path, format!("{json}\n"))?,
        None => println!("{json}"),
    }
    Ok(())
}
