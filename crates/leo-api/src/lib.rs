//! Shared chat and catalog service for the desktop bridge and `leo-server`.
//!
//! [`App`] talks to PostgreSQL and the active provider (including Ollama) through
//! `leo-store`, `leo-llm`, and `leo-tools`. [`App::start_services`] brings up
//! the local Compose stack (Postgres and MCP Toolbox). Frontend DTOs expose
//! credential availability, never stored secrets.

mod dto;
mod host;

use std::sync::Arc;

use leo_llm::ChatRequest;
use leo_store::{self as db, CHANNEL_LOCAL, CONTEXT_LIMIT, NewMessage};
use leo_tools::Registry;
use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

pub use dto::{
    ChatOut, ConversationDto, EngineDto, ModelDto, Op, ProviderDto, SnapshotDto, TurnDto,
};
pub use host::{ServiceDto, ServicesDto};

#[derive(Clone)]
pub struct App {
    inner: Arc<Inner>,
}

struct Inner {
    pool: RwLock<Option<PgPool>>,
    db_error: RwLock<Option<String>>,
    tools: Registry,
}

impl App {
    pub fn new(pool: Option<PgPool>, db_error: Option<String>, tools: Registry) -> Self {
        Self {
            inner: Arc::new(Inner {
                pool: RwLock::new(pool),
                db_error: RwLock::new(db_error),
                tools,
            }),
        }
    }

    pub fn unavailable(msg: impl Into<String>) -> Self {
        Self::new(None, Some(msg.into()), Registry::default())
    }

    /// Loads env, tools, and PostgreSQL. A missing database is stored as an
    /// error string; later calls fail with that message instead of panicking.
    pub async fn boot() -> Self {
        leo_llm::load_dotenv();
        let tools = Registry::from_env();
        let app = Self::new(None, None, tools);
        let _ = app.try_connect().await;
        app
    }

    pub async fn db_ok(&self) -> bool {
        self.inner.pool.read().await.is_some()
    }

    /// Starts Postgres and MCP Toolbox via Docker Compose, then reconnects.
    pub async fn start_services(&self) -> ServicesDto {
        let mut out = host::start().await;
        if out.error.is_some() {
            return out;
        }
        for _ in 0..25 {
            if self.try_connect().await.is_ok() {
                return host::status().await;
            }
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
        let err = self.try_connect().await.err();
        out = host::status().await;
        if let Some(err) = err {
            out.ok = false;
            out.error = Some(err);
        }
        out
    }

    pub async fn services(&self) -> ServicesDto {
        host::status().await
    }

    async fn try_connect(&self) -> Result<(), String> {
        let url = db::database_url();
        match db::connect(&url).await {
            Ok(pool) => {
                if let Err(err) = db::migrate(&pool).await {
                    let msg = format!("migrate: {err}");
                    *self.inner.pool.write().await = None;
                    *self.inner.db_error.write().await = Some(msg.clone());
                    return Err(msg);
                }
                let _ = db::apply_secrets_to_env(&pool).await;
                let _ = db::sync_ollama_providers(&pool).await;
                *self.inner.pool.write().await = Some(pool);
                *self.inner.db_error.write().await = None;
                Ok(())
            }
            Err(err) => {
                let msg = format!(
                    "no se pudo conectar a postgres ({url}): {err}. arranca los servicios desde Leo"
                );
                *self.inner.pool.write().await = None;
                *self.inner.db_error.write().await = Some(msg.clone());
                Err(msg)
            }
        }
    }

    async fn pool(&self) -> Result<PgPool, String> {
        if let Some(pool) = self.inner.pool.read().await.clone() {
            return Ok(pool);
        }
        Err(self
            .inner
            .db_error
            .read()
            .await
            .clone()
            .unwrap_or_else(|| "sin postgres".into()))
    }

    pub async fn snapshot(&self) -> Result<SnapshotDto, String> {
        let pool = self.pool().await?;
        let _ = db::sync_ollama_providers(&pool).await;
        let snap = db::load(&pool).await.map_err(|e| e.to_string())?;
        Ok(dto::snapshot_dto(snap, &self.inner.tools.names()))
    }

    pub async fn apply(&self, op: Op) -> Result<SnapshotDto, String> {
        let pool = self.pool().await?;
        let snap = db::apply(&pool, op.into())
            .await
            .map_err(|e| e.to_string())?;
        Ok(dto::snapshot_dto(snap, &self.inner.tools.names()))
    }

    pub async fn list_chats(&self) -> Result<Vec<ConversationDto>, String> {
        let pool = self.pool().await?;
        let rows = db::list_conversations(&pool, CHANNEL_LOCAL)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(dto::conversation_dto).collect())
    }

    pub async fn open_chat(&self, id: Uuid) -> Result<Vec<TurnDto>, String> {
        let pool = self.pool().await?;
        db::set_active_conversation(&pool, Some(id))
            .await
            .map_err(|e| e.to_string())?;
        let rows = db::conversation_messages(&pool, id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(dto::turn_dto).collect())
    }

    pub async fn new_chat(&self) -> Result<ConversationDto, String> {
        let pool = self.pool().await?;
        let row = db::new_local(&pool).await.map_err(|e| e.to_string())?;
        Ok(dto::conversation_dto(row))
    }

    pub async fn chat(&self, conversation_id: Uuid, text: String) -> Result<ChatOut, String> {
        let pool = self.pool().await?;
        if db::uses_ollama(&db::load(&pool).await.map_err(|e| e.to_string())?) {
            let _ = db::sync_ollama_providers(&pool).await;
        }
        let snap = db::load(&pool).await.map_err(|e| e.to_string())?;
        let client = db::client_with_pool(&snap, &pool).map_err(|e| match e {
            leo_llm::LlmError::MissingKey(var) => {
                format!("falta api key ({var}): ábrelo en catálogo")
            }
            other => other.to_string(),
        })?;
        db::append_message(&pool, conversation_id, NewMessage::user(text))
            .await
            .map_err(|e| e.to_string())?;
        let history = db::context_messages(&pool, conversation_id, CONTEXT_LIMIT)
            .await
            .map_err(|e| e.to_string())?;
        let mut req = ChatRequest::with_history(&snap.system, history);
        if snap
            .active_provider()
            .is_some_and(|p| p.kind.eq_ignore_ascii_case("grok"))
        {
            req.reasoning_effort = Some(
                if snap.settings.thinking {
                    "high"
                } else {
                    "low"
                }
                .into(),
            );
        }
        let registry = if snap.settings.tools_enabled {
            self.inner.tools.clone()
        } else {
            Registry::default()
        };
        let mut result = leo_tools::chat(&client, req.clone(), &registry).await;
        if let Err(err) = &result
            && !registry.is_empty()
            && tools_unsupported(err)
        {
            result = leo_tools::chat(&client, req, &Registry::default()).await;
        }
        match result {
            Ok(resp) => {
                db::append_message(
                    &pool,
                    conversation_id,
                    NewMessage::assistant(resp.text.clone(), snap.active_model_id),
                )
                .await
                .map_err(|e| e.to_string())?;
                Ok(ChatOut { text: resp.text })
            }
            Err(err) => {
                let _ =
                    db::append_message(&pool, conversation_id, NewMessage::error(err.to_string()))
                        .await;
                Err(err.to_string())
            }
        }
    }
}

fn tools_unsupported(err: &leo_llm::LlmError) -> bool {
    let text = err.to_string().to_ascii_lowercase();
    text.contains("does not support tools") || text.contains("does not support tool")
}

#[cfg(test)]
mod tests {
    use super::*;
    use leo_store::ProviderRow;

    fn provider(kind: &str, api_key: Option<&str>) -> ProviderRow {
        ProviderRow {
            id: Uuid::from_u128(1),
            name: kind.into(),
            kind: kind.into(),
            base_url: None,
            api_key: api_key.map(str::to_string),
        }
    }

    #[test]
    fn ollama_has_no_api_key() {
        assert_eq!(dto::key_status(&provider("ollama", None)), "none");
        assert_eq!(dto::key_status(&provider("ollama", Some("ignored"))), "db");
    }

    #[test]
    fn grok_key_status() {
        let expected = if std::env::var("XAI_API_KEY").is_ok_and(|v| !v.trim().is_empty()) {
            "env"
        } else {
            "falta"
        };
        assert_eq!(dto::key_status(&provider("grok", None)), expected);
        assert_eq!(dto::key_status(&provider("grok", Some("sk"))), "db");
    }

    #[test]
    fn codex_reports_missing_oauth_credentials() {
        assert_eq!(dto::key_status(&provider("codex", None)), "falta");
        assert_eq!(
            dto::key_status(&provider("codex", Some("oauth-json"))),
            "db"
        );
    }

    #[test]
    fn op_json_matches_frontend_tags() {
        let op: Op = serde_json::from_str(
            r#"{"op":"set_kind","id":"00000000-0000-4000-8000-000000000003","kind":"ollama"}"#,
        )
        .unwrap();
        match op {
            Op::SetKind { kind, .. } => assert_eq!(kind, "ollama"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn snapshot_dto_hides_secrets() {
        let snap = leo_store::stub_snapshot("ollama", "gemma3:latest", "hola");
        let dto = dto::snapshot_dto(snap, &["get_weather".into()]);
        assert_eq!(dto.providers[0].kind, "ollama");
        assert_eq!(dto.providers[0].key, "none");
        assert_eq!(dto.tools, ["get_weather"]);
        let json = serde_json::to_value(&dto).unwrap();
        assert!(json.get("providers").unwrap()[0].get("api_key").is_none());
        assert_eq!(json["active_model_id"].as_str().unwrap().len(), 36);
    }

    #[test]
    fn detects_ollama_models_without_tools() {
        let err = leo_llm::LlmError::Http {
            status: 400,
            body: "registry.ollama.ai/library/gemma3:4b does not support tools".into(),
        };
        assert!(tools_unsupported(&err));
        assert!(!tools_unsupported(&leo_llm::LlmError::Empty("modelo")));
    }

    #[tokio::test]
    async fn boot_without_db_is_not_ok() {
        let app = App::unavailable("sin postgres");
        assert!(!app.db_ok().await);
        let err = app.snapshot().await.unwrap_err();
        assert!(err.contains("postgres"), "{err}");
    }

    #[tokio::test]
    async fn snapshot_lists_seed_ollama_when_postgres_is_up() {
        let app = App::boot().await;
        if !app.db_ok().await {
            return;
        }
        let snap = app.snapshot().await.expect("snapshot");
        let ollama = snap
            .providers
            .iter()
            .find(|p| p.kind == "ollama")
            .expect("seed ollama");
        assert_eq!(ollama.key, "none");
        assert!(
            ollama
                .base_url
                .as_deref()
                .is_some_and(|u| u.contains("11434")),
            "{ollama:?}"
        );
    }
}
