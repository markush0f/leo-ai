//! HTTP chat server (`leo-server`).
//!
//! Serves the same catalog and conversations as the TUI, Telegram, and desktop
//! app. The browser talks to this process; this process talks to Ollama and
//! the other providers. Default bind is loopback. Binding `0.0.0.0` exposes
//! chat and tools to the local network.
//!
//! # Workspace crates
//!
//! - [`leo_api`] — snapshot, catalog mutations, and chat (tool loop included).
//!   [`leo_api::App::boot`] loads dotenv, tools, and PostgreSQL.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use leo_api::App;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "leo-server",
    about = "API HTTP de Leo: chat y catálogo para el navegador"
)]
struct Cli {
    /// Listen address (`host:port`). `LEO_HTTP_BIND` overrides this when set.
    #[arg(long, default_value = "127.0.0.1:8787")]
    bind: String,
    /// Vite `dist` directory served at `/`. `LEO_WEB_ROOT` overrides this when set.
    #[arg(long)]
    web_root: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let bind = std::env::var("LEO_HTTP_BIND")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(cli.bind);
    let web_root = std::env::var("LEO_WEB_ROOT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or(cli.web_root)
        .or_else(discover_web_root);

    let addr: SocketAddr = bind.parse().unwrap_or_else(|err| {
        eprintln!("dirección inválida ({bind}): {err}");
        std::process::exit(1);
    });
    if !addr.ip().is_loopback() {
        tracing::warn!(
            %addr,
            "escuchando fuera de loopback: cualquiera en esa red puede chatear y usar tools"
        );
    }

    let app = App::boot().await;
    if app.db_ok().await {
        tracing::info!("postgres listo");
    } else {
        tracing::warn!("postgres no disponible; /api/snapshot fallará hasta que arranque");
    }

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|err| {
            eprintln!("no se pudo escuchar en {addr}: {err}");
            std::process::exit(1);
        });
    tracing::info!(%addr, web = ?web_root, "leo-server");
    axum::serve(listener, leo_server::router(app, web_root))
        .await
        .expect("server");
}

fn discover_web_root() -> Option<PathBuf> {
    ["desktop/dist", "dist"]
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.join("index.html").is_file())
}
