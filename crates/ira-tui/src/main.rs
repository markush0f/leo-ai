//! Terminal chat binary (`ira`).
//!
//! Owns the Ratatui event loop: draw, keyboard/paste, catalog edits, and
//! background model turns. Workspace crates own persistence, provider HTTP, and
//! tool execution. This binary does not talk to the voice daemon.
//!
//! # Workspace crates
//!
//! - [`ira_store`] — PostgreSQL catalog (providers, models, engines, settings,
//!   secrets) and the local conversation used by this UI. `connect` / `migrate`
//!   prepare the schema; `apply_secrets_to_env` exports stored keys; `load`
//!   returns an in-memory [`ira_store::Snapshot`]. `Snapshot::client` builds
//!   the active model's HTTP client. Chat history is `ensure_local` (or
//!   `new_local`), `context_messages` (capped by `CONTEXT_LIMIT`), and
//!   `append_message`. Catalog edits go through [`ira_store::DbOp`] / `apply`.
//!   `sync_ollama_providers` refreshes discovered Ollama models; a down
//!   provider is skipped.
//! - [`ira_llm`] — provider-independent [`ira_llm::Client`],
//!   [`ira_llm::ChatRequest`], and message types. `load_dotenv` fills missing
//!   environment keys without overwriting exported variables. This crate
//!   transports tool calls as data; it never executes them.
//! - [`ira_tools`] — [`ira_tools::Registry::from_env`] registers files, shell,
//!   system, and weather always, plus AppFlowy / GitHub / Google / Home
//!   Assistant when credentials exist, and local MCP Toolbox database tools
//!   when `MCP_TOOLBOX_URL` is set. [`ira_tools::chat`] runs the tool loop
//!   (at most eight model rounds) and returns a final reply or
//!   [`ira_llm::LlmError::ToolLoop`].
//!
//! # Local modules
//!
//! - `app`: UI state, pending chat/DB work, and conversation bubbles.
//! - `input`: line editor used by chat and settings fields.
//! - `slash`: `/` command palette over the current snapshot.
//! - `settings`: catalog editor; emits `DbOp` values for `apply`.
//! - `ui`: Ratatui layout for chat and settings.
//!
//! # Startup
//!
//! Database URL: `--database-url`, else `IRA_DATABASE_URL` / `DATABASE_URL`,
//! else `postgres://ira:ira@127.0.0.1:5439/ira?sslmode=disable`. A missing
//! API key is a UI error, not a process exit.

mod app;
mod input;
mod mcp_cli;
mod settings;
mod slash;
mod ui;

use std::io::stdout;

use clap::{Parser, Subcommand};
use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, Event, EventStream, KeyEventKind,
};
use crossterm::execute;
use futures::StreamExt;
use ira_llm::Client;
use tokio::sync::mpsc;

use crate::app::App;
use ira_llm::{ChatMessage, Role};
use ira_store::{
    self as db, CONTEXT_LIMIT, NewMessage, Snapshot, conversation_messages, database_url,
    ensure_local, new_local,
};

#[derive(Parser)]
#[command(name = "ira", about = "Pregúntale al LLM desde la terminal")]
struct Cli {
    /// PostgreSQL URL override; otherwise resolved from environment or local defaults.
    #[arg(long, global = true)]
    database_url: Option<String>,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Instala, registra y lista servidores MCP.
    Mcp {
        #[command(subcommand)]
        cmd: mcp_cli::McpCmd,
    },
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
    ira_llm::load_dotenv();
    if let Err(err) = run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    if let Some(Cmd::Mcp { cmd }) = cli.cmd {
        return mcp_cli::run(cli.database_url, cmd).await;
    }
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
    let _ = db::apply_secrets_to_env(&pool).await;
    let _ = db::sync_ollama_providers(&pool).await;
    let snapshot = db::load(&pool).await?;
    let conv = ensure_local(&pool).await?;
    let messages = conversation_messages(&pool, conv.id).await?;

    let mut client = try_client(&snapshot, &pool);
    let tools = ira_tools::Registry::from_env();
    let tools = ira_tools::attach_configured(tools, &ira_tools::file_servers()).await;
    let tools = ira_tools::attach_memory(tools, pool.clone());
    let mut app = App::from_store(snapshot, conv.id, messages);
    let mut terminal = ratatui::init();
    let _restore = Restore;
    execute!(stdout(), EnableBracketedPaste)?;
    terminal.clear()?;

    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut events = EventStream::new();

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        if app.take_new_conversation() {
            match new_local(&pool).await {
                Ok(conv) => {
                    app.conversation_id = conv.id;
                    app.history.clear();
                    app.bubbles.clear();
                }
                Err(err) => app.push_error(err),
            }
        }

        if let Some(req) = app.take_pending_chat() {
            if db::uses_ollama(&app.snapshot) {
                refresh_ollama(&pool, &mut app, &mut client).await;
            }
            let tools_for_turn = ira_tools::attach_memory(tools.clone(), pool.clone());
            let names = tools_for_turn.names();
            let req = match persist_user_and_context(&pool, &app, req, &names).await {
                Ok(req) => req,
                Err(err) => {
                    app.busy = false;
                    app.push_error(err);
                    continue;
                }
            };
            match client.clone() {
                Some(client) => {
                    let tx = tx.clone();
                    let tools = tools_for_turn;
                    tokio::spawn(async move {
                        let _ = tx.send(ira_tools::chat(&client, req, &tools).await);
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
                    client = try_client(&snap, &pool);
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
                    persist_reply(&pool, app.conversation_id, app.snapshot.active_model_id, &result)
                        .await;
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
        *client = try_client(&snap, pool);
        app.apply_snapshot(snap);
    }
}

fn try_client(snapshot: &Snapshot, pool: &sqlx::PgPool) -> Option<Client> {
    db::client_with_pool(snapshot, pool).ok()
}

async fn persist_user_and_context(
    pool: &sqlx::PgPool,
    app: &App,
    req: ira_llm::ChatRequest,
    tool_names: &[String],
) -> Result<ira_llm::ChatRequest, sqlx::Error> {
    if app.conversation_id.is_nil() {
        return Ok(req);
    }
    if let Some(ChatMessage { content, role, .. }) = app.history.last()
        && *role == Role::User
    {
        db::append_message(pool, app.conversation_id, NewMessage::user(content.clone())).await?;
    }
    let history = db::context_messages(pool, app.conversation_id, CONTEXT_LIMIT).await?;
    let user_text = history
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .map(|message| message.content.as_str())
        .unwrap_or("");
    let memory = db::prepare_turn(pool, user_text)
        .await
        .map_err(|err| sqlx::Error::Protocol(err.to_string()))?
        .prompt;
    let model = app
        .snapshot
        .active_model()
        .map(|model| model.name.clone())
        .unwrap_or_else(|| "desconocido".into());
    let system = db::compose_prompt(
        pool,
        "chat",
        &db::PromptDynamic {
            model: &model,
            tool_names,
            memories: &memory,
            extra: "",
        },
    )
    .await?;
    let mut req = ira_llm::ChatRequest::with_history(&system, history);
    app.snapshot.apply_reasoning(&mut req);
    Ok(req)
}

async fn persist_reply(
    pool: &sqlx::PgPool,
    conversation_id: uuid::Uuid,
    model_id: Option<uuid::Uuid>,
    result: &Result<ira_llm::ChatResponse, ira_llm::LlmError>,
) {
    if conversation_id.is_nil() {
        return;
    }
    let msg = match result {
        Ok(resp) => NewMessage::assistant(resp.text.clone(), model_id),
        Err(err) => NewMessage::error(err.to_string()),
    };
    let _ = db::append_message(pool, conversation_id, msg).await;
}
