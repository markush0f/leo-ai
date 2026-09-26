//! Telegram long-polling binary (`ira-telegram`).
//!
//! Pulls Bot API updates, enforces an explicit user allowlist, and applies
//! library [`ira_telegram::Outcome`] values: send text, persist catalog edits,
//! start a new conversation, or run a tool-enabled model turn. An empty
//! allowlist accepts no users. This process does not serve voice IPC.
//!
//! # Workspace crates
//!
//! - [`ira_store`] — shared PostgreSQL catalog and per-chat Telegram history.
//!   `connect` / `migrate` prepare the schema; `apply_secrets_to_env` exports
//!   stored keys; `telegram_token` and settings overlay CLI/env when those
//!   are empty. `ensure_telegram` / `new_telegram` own the conversation for a
//!   chat id; `context_messages` / `append_message` persist turns.
//!   [`ira_store::DbOp`] / `apply` handle `/model`, `/providers`, and
//!   `/system`. `sync_ollama_providers` runs before Ollama turns.
//! - [`ira_llm`] — [`ira_llm::Client`] via `Snapshot::client`, plus
//!   [`ira_llm::ChatRequest`] / [`ira_llm::ChatMessage`]. `load_dotenv` fills
//!   missing environment keys. Tool calls are data; this crate does not run
//!   them.
//! - [`ira_tools`] — [`ira_tools::Registry::from_env`] and [`ira_tools::chat`]
//!   execute requested tools and feed results back until a final reply or
//!   [`ira_llm::LlmError::ToolLoop`].
//! - this crate's library (`ira_telegram`): command routing and in-memory
//!   [`ira_telegram::Session`]. [`ira_telegram::on_text`] returns outcomes;
//!   it does not call Telegram HTTP. Plain text reaches the model only in
//!   private chats; groups are command-only. Authorization is applied here
//!   before routing.
//!
//! # Local modules
//!
//! - `config`: token, allowlist, and database URL from flags, env, optional
//!   TOML, then Postgres overlay.
//! - `tg`: Bot API transport (`getUpdates`, `sendMessage`, typing). Splits
//!   replies with [`ira_telegram::split_telegram`].
//!
//! # Startup
//!
//! Token: `--token`, else `TELEGRAM_BOT_TOKEN`, else config file, else the
//! `telegram_bot_token` secret. Database URL follows the same store
//! precedence as the TUI. Missing token exits; empty allowlist warns and
//! stays idle.

mod config;
mod tg;

use std::collections::HashMap;
use std::time::Duration;

use clap::Parser;
use ira_llm::{ChatMessage, ChatRequest};
use ira_store::{self as store, CONTEXT_LIMIT, NewMessage, Snapshot, telegram_token};
use tracing_subscriber::EnvFilter;

use crate::config::BotConfig;
use crate::tg::Telegram;
use ira_telegram::{Outcome, Session, allowed, cap_history, on_text};

#[derive(Parser)]
#[command(name = "ira-telegram", about = "Pregúntale al LLM desde Telegram")]
struct Cli {
    /// PostgreSQL URL override; otherwise resolved from environment or local defaults.
    #[arg(long)]
    database_url: Option<String>,
    /// Bot token override; otherwise read from TELEGRAM_BOT_TOKEN.
    #[arg(long)]
    token: Option<String>,
}

#[tokio::main]
async fn main() {
    ira_llm::load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    if let Err(err) = run().await {
        tracing::error!(%err, "ira-telegram");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut cfg = BotConfig::load(cli.token, cli.database_url)?;
    let pool = match store::connect(&cfg.database_url).await {
        Ok(pool) => pool,
        Err(err) => {
            return Err(format!(
                "no se pudo conectar a postgres ({}): {err}\narranca la bbdd: docker compose up -d",
                cfg.database_url
            )
            .into());
        }
    };
    store::migrate(&pool).await?;
    let _ = store::apply_secrets_to_env(&pool).await;
    let _ = store::sync_ollama_providers(&pool).await;
    overlay_bot_config(&pool, &mut cfg).await?;
    if cfg.token.trim().is_empty() {
        return Err("falta TELEGRAM_BOT_TOKEN (en postgres, .env, el entorno o --token)".into());
    }
    if cfg.allow_users.is_empty() {
        tracing::warn!(
            "allowlist vacía: el bot no habla con nadie hasta que pongas tu user id en settings o TELEGRAM_ALLOW_USERS"
        );
    }

    let tools = ira_tools::Registry::from_env();
    let tg = Telegram::new(&cfg.token)?;
    let me = tg.get_me().await?;
    tracing::info!(user = %me, "bot");

    let mut offset: i64 = 0;
    let mut sessions: HashMap<i64, Session> = HashMap::new();

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("salida");
                break;
            }
            result = tg.get_updates(offset) => {
                let updates = match result {
                    Ok(u) => u,
                    Err(err) => {
                        tracing::warn!(%err, "getUpdates");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                };
                for update in updates {
                    offset = offset.max(update.update_id + 1);
                    let Some(msg) = update.message else {
                        continue;
                    };
                    let chat_id = msg.chat.id;
                    let private = msg.chat.is_private();
                    let user_id = msg.from.as_ref().map(|u| u.id).unwrap_or(chat_id);
                    let Some(text) = msg.text.as_deref() else {
                        if private && allowed(&cfg.allow_users, user_id) {
                            let _ = tg.send_text(chat_id, "manda texto. /help").await;
                        }
                        continue;
                    };
                    if !allowed(&cfg.allow_users, user_id) {
                        tracing::info!(user_id, "rechazado");
                        if private {
                            let _ = tg
                                .send_text(
                                    chat_id,
                                    &format!(
                                        "no estás en la lista. tu id es {user_id}.\n\
                                         guarda TELEGRAM_ALLOW_USERS={user_id} o ponlo en settings"
                                    ),
                                )
                                .await;
                        }
                        continue;
                    }

                    let mut snap = refresh(&pool).await?;
                    let session = match load_session(&pool, &mut sessions, chat_id).await {
                        Ok(s) => s,
                        Err(err) => {
                            tracing::warn!(%err, "sesión");
                            continue;
                        }
                    };
                    match on_text(session, &snap, text, private) {
                        Outcome::Ignore => {}
                        Outcome::Text(reply) => tg.send_text(chat_id, &reply).await?,
                        Outcome::Clear => {
                            let conv = store::new_telegram(&pool, chat_id).await?;
                            session.conversation_id = conv.id;
                            session.history.clear();
                            tg.send_text(chat_id, "conversación nueva").await?;
                        }
                        Outcome::Db { op, note } => {
                            match store::apply(&pool, op).await {
                                Ok(next) => {
                                    snap = next;
                                    tg.send_text(chat_id, &format!("{note}\n{}", ira_telegram::status_line(&snap)))
                                        .await?;
                                }
                                Err(err) => {
                                    tg.send_text(chat_id, &format!("bbdd: {err}")).await?;
                                }
                            }
                        }
                        Outcome::AskLlm => {
                            reply_llm(&tg, &pool, &snap, session, chat_id, &tools).await?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

async fn refresh(pool: &sqlx::PgPool) -> Result<Snapshot, sqlx::Error> {
    let snap = store::load(pool).await?;
    if store::uses_ollama(&snap) {
        let _ = store::sync_ollama_providers(pool).await;
        return store::load(pool).await;
    }
    Ok(snap)
}

async fn reply_llm(
    tg: &Telegram,
    pool: &sqlx::PgPool,
    snap: &Snapshot,
    session: &mut Session,
    chat_id: i64,
    tools: &ira_tools::Registry,
) -> Result<(), Box<dyn std::error::Error>> {
    if snap.active_model().is_none() {
        session.history.pop();
        tg.send_text(chat_id, "elige un modelo: /model").await?;
        return Ok(());
    }
    let client = match store::client_with_pool(snap, pool) {
        Ok(c) => c,
        Err(err) => {
            session.history.pop();
            tg.send_text(chat_id, &err.to_string()).await?;
            return Ok(());
        }
    };
    let _typing = tg.keep_typing(chat_id);
    if store::uses_ollama(snap) {
        let _ = store::sync_ollama_providers(pool).await;
    }
    if let Some(ChatMessage { content, .. }) = session.history.last() {
        store::append_message(
            pool,
            session.conversation_id,
            NewMessage::user(content.clone()),
        )
        .await?;
    }
    let history = store::context_messages(pool, session.conversation_id, CONTEXT_LIMIT).await?;
    session.history = history.clone();
    let mut req = ChatRequest::with_history(&snap.system, history);
    snap.apply_reasoning(&mut req);
    match ira_tools::chat(&client, req, tools).await {
        Ok(resp) => {
            store::append_message(
                pool,
                session.conversation_id,
                NewMessage::assistant(resp.text.clone(), snap.active_model_id),
            )
            .await?;
            session
                .history
                .push(ChatMessage::assistant(resp.text.clone()));
            cap_history(&mut session.history);
            tg.send_text(chat_id, &resp.text).await?;
        }
        Err(err) => {
            session.history.pop();
            let _ = store::append_message(
                pool,
                session.conversation_id,
                NewMessage::error(err.to_string()),
            )
            .await;
            tg.send_text(chat_id, &err.to_string()).await?;
        }
    }
    Ok(())
}

async fn overlay_bot_config(pool: &sqlx::PgPool, cfg: &mut BotConfig) -> Result<(), sqlx::Error> {
    if cfg.token.is_empty() {
        if let Some(token) = telegram_token(pool).await? {
            cfg.token = token;
        }
    }
    let snap = store::load(pool).await?;
    if cfg.allow_users.is_empty() && !snap.settings.telegram_allow_users.is_empty() {
        cfg.allow_users = snap.settings.telegram_allow_users.clone();
    }
    Ok(())
}

async fn load_session<'a>(
    pool: &sqlx::PgPool,
    sessions: &'a mut HashMap<i64, Session>,
    chat_id: i64,
) -> Result<&'a mut Session, sqlx::Error> {
    if !sessions.contains_key(&chat_id) {
        let conv = store::ensure_telegram(pool, chat_id).await?;
        let history = store::context_messages(pool, conv.id, CONTEXT_LIMIT).await?;
        sessions.insert(
            chat_id,
            Session {
                conversation_id: conv.id,
                history,
            },
        );
    }
    Ok(sessions.get_mut(&chat_id).expect("just inserted"))
}
