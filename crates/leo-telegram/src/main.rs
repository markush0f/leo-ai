mod config;
mod tg;

use std::collections::HashMap;
use std::time::Duration;

use clap::Parser;
use leo_llm::{ChatMessage, ChatRequest};
use leo_store::{self as store, Snapshot};
use tracing_subscriber::EnvFilter;

use crate::config::BotConfig;
use crate::tg::Telegram;
use leo_telegram::{allowed, cap_history, on_text, Outcome, Session};

#[derive(Parser)]
#[command(name = "leo-telegram", about = "Pregúntale al LLM desde Telegram")]
struct Cli {
    /// URL de Postgres (si no, DATABASE_URL en .env)
    #[arg(long)]
    database_url: Option<String>,
    /// Token del bot (si no, TELEGRAM_BOT_TOKEN en .env)
    #[arg(long)]
    token: Option<String>,
}

#[tokio::main]
async fn main() {
    leo_llm::load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    if let Err(err) = run().await {
        tracing::error!(%err, "leo-telegram");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let cfg = BotConfig::load(cli.token, cli.database_url)?;
    if cfg.allow_users.is_empty() {
        tracing::warn!(
            "TELEGRAM_ALLOW_USERS vacío: el bot no habla con nadie hasta que pongas tu user id"
        );
    }

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
    let _ = store::sync_ollama_providers(&pool).await;

    let tools = leo_tools::Registry::from_env();
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
                                         export TELEGRAM_ALLOW_USERS={user_id}"
                                    ),
                                )
                                .await;
                        }
                        continue;
                    }

                    let mut snap = refresh(&pool).await?;
                    let session = sessions.entry(chat_id).or_default();
                    match on_text(session, &snap, text, private) {
                        Outcome::Ignore => {}
                        Outcome::Text(reply) => tg.send_text(chat_id, &reply).await?,
                        Outcome::Db { op, note } => {
                            match store::apply(&pool, op).await {
                                Ok(next) => {
                                    snap = next;
                                    tg.send_text(chat_id, &format!("{note}\n{}", leo_telegram::status_line(&snap)))
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
    tools: &leo_tools::Registry,
) -> Result<(), Box<dyn std::error::Error>> {
    if snap.active_model().is_none() {
        session.history.pop();
        tg.send_text(chat_id, "elige un modelo: /model").await?;
        return Ok(());
    }
    let client = match snap.client() {
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
    let req = ChatRequest::with_history(&snap.system, session.history.clone());
    match leo_tools::chat(&client, req, tools).await {
        Ok(resp) => {
            session.history.push(ChatMessage::assistant(resp.text.clone()));
            cap_history(&mut session.history);
            tg.send_text(chat_id, &resp.text).await?;
        }
        Err(err) => {
            session.history.pop();
            tg.send_text(chat_id, &err.to_string()).await?;
        }
    }
    Ok(())
}
