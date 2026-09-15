//! Terminal chat entry point.
//!
//! Loads the shared PostgreSQL catalog and dispatches model turns asynchronously.
//! `app` owns UI state, `input` and `slash` interpret input, `settings` manages
//! catalog editing, and `ui` renders the current state with Ratatui.

mod app;
mod input;
mod settings;
mod slash;
mod ui;

use std::io::stdout;

use clap::Parser;
use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, Event, EventStream, KeyEventKind,
};
use crossterm::execute;
use futures::StreamExt;
use leo_llm::Client;
use tokio::sync::mpsc;

use crate::app::App;
use leo_store::{self as db, Snapshot, database_url};

#[derive(Parser)]
#[command(name = "leo", about = "Pregúntale al LLM desde la terminal")]
struct Cli {
    /// PostgreSQL URL override; otherwise resolved from environment or local defaults.
    #[arg(long)]
    database_url: Option<String>,
}

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = execute!(stdout(), DisableBracketedPaste);
        ratatui::restore();
    }
}

#[tokio::main]
async fn main() {
    leo_llm::load_dotenv();
    if let Err(err) = run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let url = cli.database_url.unwrap_or_else(database_url);
    let pool = match db::connect(&url).await {
        Ok(pool) => pool,
        Err(err) => {
            return Err(format!(
                "no se pudo conectar a postgres ({url}): {err}\narranca la bbdd: docker compose up -d"
            )
            .into());
        }
    };
    db::migrate(&pool).await?;
    let _ = db::sync_ollama_providers(&pool).await;
    let snapshot = db::load(&pool).await?;

    let mut client = try_client(&snapshot);
    let tools = leo_tools::Registry::from_env();
    let mut app = App::new(snapshot);
    let mut terminal = ratatui::init();
    let _restore = Restore;
    execute!(stdout(), EnableBracketedPaste)?;
    terminal.clear()?;

    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut events = EventStream::new();

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        if let Some(req) = app.take_pending_chat() {
            if db::uses_ollama(&app.snapshot) {
                refresh_ollama(&pool, &mut app, &mut client).await;
            }
            match client.clone() {
                Some(client) => {
                    let tx = tx.clone();
                    let tools = tools.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(leo_tools::chat(&client, req, &tools).await);
                    });
                }
                None => {
                    app.busy = false;
                    app.push_error("falta api key: tab → opciones, o exporta XAI_API_KEY");
                }
            }
        }

        if let Some(op) = app.take_pending_db() {
            match db::apply(&pool, op).await {
                Ok(snap) => {
                    client = try_client(&snap);
                    app.apply_snapshot(snap);
                }
                Err(err) => app.on_db_err(err),
            }
        }

        if app.should_quit {
            break;
        }

        tokio::select! {
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key)))
                        if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat =>
                    {
                        let was_slash = app.slash_open();
                        app.on_key(key);
                        if app.slash_open() && !was_slash {
                            refresh_ollama(&pool, &mut app, &mut client).await;
                        }
                    }
                    Some(Ok(Event::Paste(text))) => {
                        let was_slash = app.slash_open();
                        app.on_paste(text);
                        if app.slash_open() && !was_slash {
                            refresh_ollama(&pool, &mut app, &mut client).await;
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => {}
                    Some(Err(err)) => return Err(err.into()),
                    None => break,
                    _ => {}
                }
            }
            maybe_reply = rx.recv() => {
                if let Some(result) = maybe_reply {
                    app.on_reply(result);
                }
            }
        }
    }

    Ok(())
}

async fn refresh_ollama(pool: &sqlx::PgPool, app: &mut App, client: &mut Option<Client>) {
    if db::sync_ollama_providers(pool).await.is_err() {
        return;
    }
    if let Ok(snap) = db::load(pool).await {
        *client = try_client(&snap);
        app.apply_snapshot(snap);
    }
}

fn try_client(snapshot: &Snapshot) -> Option<Client> {
    snapshot.client().ok()
}
