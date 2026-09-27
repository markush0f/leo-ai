use std::path::PathBuf;

use clap::Subcommand;
use ira_mcp::Scope;

#[derive(Subcommand)]
pub enum McpCmd {
    /// Registra un MCP cuyo comando o URL ya conoces.
    Add {
        name: String,
        /// Endpoint remoto.
        #[arg(long)]
        url: Option<String>,
        /// Escribe en `ira.json` del proyecto, no en el global.
        #[arg(long)]
        project: bool,
        /// Comando local tras `--`.
        #[arg(last = true)]
        command: Vec<String>,
    },
    /// Descubre cómo instalarlo y lo escribe en `ira.json`.
    Install {
        name: String,
        #[arg(long)]
        project: bool,
    },
    /// Importa un JSON `mcpServers` estilo Claude.
    Import {
        path: PathBuf,
        #[arg(long)]
        project: bool,
    },
    /// Lista los MCP de `ira.json` (global y proyecto).
    List,
    /// Borra un MCP por nombre.
    Remove { name: String },
}

pub async fn run(
    _database_url: Option<String>,
    cmd: McpCmd,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        McpCmd::Add {
            name,
            url,
            project,
            command,
        } => {
            let config = ira_mcp::from_add(&name, url.as_deref(), &command)?;
            let path = ira_mcp::upsert(scope(project), &config)?;
            println!("{}  {}", path.display(), describe(&config));
        }
        McpCmd::Install { name, project } => {
            let config = ira_mcp::resolve(&name).await?;
            let path = ira_mcp::upsert(scope(project), &config)?;
            println!("{}  {}", path.display(), describe(&config));
        }
        McpCmd::Import { path, project } => {
            let configs = ira_mcp::load_file(&path).await?;
            if configs.is_empty() {
                return Err("mcpServers vacío".into());
            }
            let target = scope(project);
            for config in &configs {
                let written = ira_mcp::upsert(target, config)?;
                println!("{}  {}", written.display(), describe(config));
            }
        }
        McpCmd::List => {
            println!("{}", ira_mcp::global_path().display());
            if let Some(path) = ira_mcp::project_file() {
                if path != ira_mcp::global_path() {
                    println!("{}", path.display());
                }
            }
            let rows = ira_mcp::load_all()?;
            if rows.is_empty() {
                println!("sin MCP");
                return Ok(());
            }
            for row in rows {
                let state = if row.enabled { "on" } else { "off" };
                println!("{state}  {}", describe(&row));
            }
        }
        McpCmd::Remove { name } => {
            ira_mcp::shared().disconnect(&name).await;
            let paths = ira_mcp::remove(&name)?;
            for path in paths {
                println!("borrado {name} de {}", path.display());
            }
        }
    }
    Ok(())
}

fn scope(project: bool) -> Scope {
    if project {
        Scope::Project
    } else {
        Scope::Global
    }
}

fn describe(config: &ira_mcp::McpServerConfig) -> String {
    match config.transport {
        ira_mcp::McpTransport::Stdio => format!(
            "{}  local  {} {}",
            config.name,
            config.command.as_deref().unwrap_or(""),
            config.args.join(" ")
        ),
        ira_mcp::McpTransport::StreamableHttp => format!(
            "{}  remote  {}",
            config.name,
            config.url.as_deref().unwrap_or("")
        ),
    }
}
