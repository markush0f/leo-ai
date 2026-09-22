//! Shared chat and catalog service for the desktop bridge and `ira-server`.
//!
//! [`App`] talks to PostgreSQL and the active provider (including Ollama) through
//! `ira-store`, `ira-llm`, and `ira-tools`. [`App::start_services`] brings up
//! the local Compose stack (Postgres and MCP Toolbox). Frontend DTOs expose
//! credential availability, never stored secrets.

mod dto;
mod host;
mod toolbox;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ira_llm::ChatRequest;
use ira_llm::providers::codex::{DeviceLogin, OAuthClient, TokenStore};
use ira_store::{self as db, CHANNEL_LOCAL, CONTEXT_LIMIT, NewMessage, PostgresCodexTokenStore};
use ira_tools::Registry;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

pub use dto::{
    ChatOut, CodexLoginDto, ConversationDto, DatabaseConnectionDto, DatabaseExportRequest,
    DatabaseInput, DatabaseTestDto, DeleteDto, EngineDto, ModelDto, Op, ProviderDto, SnapshotDto,
    TurnDto,
};
pub use host::{ServiceDto, ServicesDto};
pub use ira_pgjson::Dump as DatabaseDump;

#[derive(Clone)]
pub struct App {
    inner: Arc<Inner>,
}

struct Inner {
    pool: RwLock<Option<PgPool>>,
    db_error: RwLock<Option<String>>,
    tools: Registry,
    toolbox_sync: Mutex<()>,
    codex_logins: Mutex<HashMap<Uuid, PendingCodexLogin>>,
}

#[derive(Clone)]
struct PendingCodexLogin {
    provider_id: Uuid,
    login: DeviceLogin,
}

impl App {
    pub fn new(pool: Option<PgPool>, db_error: Option<String>, tools: Registry) -> Self {
        Self {
            inner: Arc::new(Inner {
                pool: RwLock::new(pool),
                db_error: RwLock::new(db_error),
                tools,
                toolbox_sync: Mutex::new(()),
                codex_logins: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn unavailable(msg: impl Into<String>) -> Self {
        Self::new(None, Some(msg.into()), Registry::default())
    }

    /// Loads env, tools, and PostgreSQL. A missing database is stored as an
    /// error string; later calls fail with that message instead of panicking.
    pub async fn boot() -> Self {
        ira_llm::load_dotenv();
        let tools = Registry::from_env();
        if let Err(error) = toolbox::reset().await {
            return Self::new(None, Some(error), tools);
        }
        let app = Self::new(None, None, tools);
        let _ = app.try_connect().await;
        app
    }

    pub async fn db_ok(&self) -> bool {
        self.inner.pool.read().await.is_some()
    }

    /// Starts Postgres and MCP Toolbox via Docker Compose, then reconnects.
    pub async fn start_services(&self) -> ServicesDto {
        if let Err(error) = toolbox::reset().await {
            let mut out = host::status().await;
            out.ok = false;
            out.error = Some(error);
            return out;
        }
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
                let _ = db::sync_codex_providers(&pool).await;
                *self.inner.pool.write().await = Some(pool.clone());
                *self.inner.db_error.write().await = None;
                if let Ok(cipher) = db::DatabaseCipher::from_env() {
                    let app = self.clone();
                    tokio::spawn(async move {
                        let _ = app.sync_toolbox(&pool, &cipher).await;
                    });
                } else {
                    if let Err(error) = toolbox::reset().await {
                        *self.inner.pool.write().await = None;
                        *self.inner.db_error.write().await = Some(error.clone());
                        return Err(error);
                    }
                }
                Ok(())
            }
            Err(err) => {
                let msg = format!(
                    "no se pudo conectar a postgres ({url}): {err}. arranca los servicios desde Ira"
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

    pub async fn list_databases(&self) -> Result<Vec<DatabaseConnectionDto>, String> {
        let pool = self.pool().await?;
        let rows = db::list_database_connections(&pool)
            .await
            .map_err(|err| err.to_string())?;
        Ok(rows.into_iter().map(dto::database_dto).collect())
    }

    pub async fn create_database(
        &self,
        input: DatabaseInput,
    ) -> Result<DatabaseConnectionDto, String> {
        let pool = self.pool().await?;
        let input = validate_database_input(input, true)?;
        let cipher = db::DatabaseCipher::from_env().map_err(|err| err.to_string())?;
        let row = db::create_database_connection(&pool, &cipher, database_write(input))
            .await
            .map_err(|err| err.to_string())?;
        if let Err(error) = self.sync_toolbox(&pool, &cipher).await {
            let _ = db::set_database_test_result(&pool, row.id, false, Some(&error)).await;
        }
        let row = db::database_connection(&pool, row.id)
            .await
            .map_err(|err| err.to_string())?;
        Ok(dto::database_dto(row))
    }

    pub async fn update_database(
        &self,
        id: Uuid,
        input: DatabaseInput,
    ) -> Result<DatabaseConnectionDto, String> {
        let pool = self.pool().await?;
        let input = validate_database_input(input, false)?;
        let cipher = db::DatabaseCipher::from_env().map_err(|err| err.to_string())?;
        let row = db::update_database_connection(&pool, &cipher, id, database_write(input))
            .await
            .map_err(|err| err.to_string())?;
        if let Err(error) = self.sync_toolbox(&pool, &cipher).await {
            let _ = db::set_database_test_result(&pool, row.id, false, Some(&error)).await;
        }
        let row = db::database_connection(&pool, row.id)
            .await
            .map_err(|err| err.to_string())?;
        Ok(dto::database_dto(row))
    }

    pub async fn delete_database(&self, id: Uuid) -> Result<DeleteDto, String> {
        let pool = self.pool().await?;
        let cipher = db::DatabaseCipher::from_env().map_err(|err| err.to_string())?;
        match db::delete_database_connection(&pool, id).await {
            Ok(()) | Err(db::DatabaseError::NotFound) => {}
            Err(error) => return Err(error.to_string()),
        }
        self.sync_toolbox(&pool, &cipher).await?;
        Ok(DeleteDto { ok: true })
    }

    /// Lee las tablas de la conexión guardada `id` y las devuelve en JSON.
    pub async fn export_database(
        &self,
        id: Uuid,
        request: DatabaseExportRequest,
    ) -> Result<DatabaseDump, String> {
        let pool = self.pool().await?;
        let row = db::database_connection(&pool, id)
            .await
            .map_err(|err| err.to_string())?;
        let cipher = db::DatabaseCipher::from_env().map_err(|err| err.to_string())?;
        let password = db::database_password(&pool, &cipher, id)
            .await
            .map_err(|err| err.to_string())?;
        if password.is_empty() {
            return Err("la conexión no tiene contraseña".into());
        }
        let port = u16::try_from(row.port)
            .ok()
            .filter(|port| *port > 0)
            .ok_or_else(|| "puerto fuera de rango (1-65535)".to_string())?;
        let connection = ira_pgjson::DbConnection {
            host: database_host(&row.host).to_string(),
            port,
            database: row.database,
            username: row.username,
            password,
            ssl_mode: ira_pgjson::SslMode::parse(&row.ssl_mode).map_err(|err| err.to_string())?,
        };
        let options = connection.to_options().map_err(|err| err.to_string())?;
        let export = ira_pgjson::ExportOptions {
            schemas: filled(request.schemas),
            tables: filled(request.tables),
            limit: request.limit,
            include_views: request.views,
            schema_only: request.schema_only,
        };
        ira_pgjson::export(options, &export)
            .await
            .map_err(|err| err.to_string())
    }

    pub async fn test_database(&self, id: Uuid) -> Result<DatabaseTestDto, String> {
        let pool = self.pool().await?;
        let row = db::database_connection(&pool, id)
            .await
            .map_err(|err| err.to_string())?;
        let cipher = db::DatabaseCipher::from_env().map_err(|err| err.to_string())?;
        let password = db::database_password(&pool, &cipher, id)
            .await
            .map_err(|err| err.to_string())?;
        if password.is_empty() {
            return Err("la conexión no tiene contraseña".into());
        }
        let result = test_postgres(&row, &password).await;
        match result {
            Ok(read_only) => {
                let detail = if read_only {
                    "Conexión correcta; sesión PostgreSQL en solo lectura."
                } else {
                    "Conexión correcta; PostgreSQL no confirma solo lectura. Usa un usuario sin permisos de escritura."
                };
                db::set_database_test_result(&pool, id, true, None)
                    .await
                    .map_err(|err| err.to_string())?;
                if let Err(error) = self.sync_toolbox(&pool, &cipher).await {
                    return Ok(DatabaseTestDto {
                        ok: false,
                        read_only,
                        detail: format!(
                            "La base responde, pero Toolbox no pudo publicarla: {error}"
                        ),
                    });
                }
                Ok(DatabaseTestDto {
                    ok: true,
                    read_only,
                    detail: detail.into(),
                })
            }
            Err(error) => {
                db::set_database_test_result(&pool, id, false, Some(&error))
                    .await
                    .map_err(|err| err.to_string())?;
                if let Err(sync_error) = self.sync_toolbox(&pool, &cipher).await {
                    return Ok(DatabaseTestDto {
                        ok: false,
                        read_only: false,
                        detail: format!(
                            "{error}. Además, Toolbox no pudo reconciliarse: {sync_error}"
                        ),
                    });
                }
                Ok(DatabaseTestDto {
                    ok: false,
                    read_only: false,
                    detail: error,
                })
            }
        }
    }

    async fn sync_toolbox(&self, pool: &PgPool, cipher: &db::DatabaseCipher) -> Result<(), String> {
        let _guard = self.inner.toolbox_sync.lock().await;
        let rows = db::list_database_connections(pool)
            .await
            .map_err(|err| err.to_string())?;
        for row in rows.into_iter().filter(|row| row.enabled) {
            let password = match db::database_password(pool, cipher, row.id).await {
                Ok(password) if !password.is_empty() => password,
                Ok(_) => {
                    db::set_database_test_result(pool, row.id, false, Some("falta contraseña"))
                        .await
                        .map_err(|err| err.to_string())?;
                    continue;
                }
                Err(error) => {
                    let error = error.to_string();
                    self.mark_enabled_database_error(pool, &error).await?;
                    toolbox::reset().await?;
                    return Err(error);
                }
            };
            match test_postgres(&row, &password).await {
                Ok(_) => db::set_database_test_result(pool, row.id, true, None)
                    .await
                    .map_err(|err| err.to_string())?,
                Err(error) => {
                    db::set_database_test_result(pool, row.id, false, Some(&error))
                        .await
                        .map_err(|err| err.to_string())?;
                }
            }
        }
        let has_active = db::list_database_connections(pool)
            .await
            .map_err(|err| err.to_string())?
            .into_iter()
            .any(|row| row.enabled && row.last_test_ok == Some(true));
        if !has_active {
            return toolbox::reset().await;
        }
        let publish = async {
            toolbox::reset().await?;
            wait_for_toolbox(&[], true).await?;
            toolbox::sync(pool, cipher).await?;
            let expected: Vec<String> = db::list_database_connections(pool)
                .await
                .map_err(|err| err.to_string())?
                .into_iter()
                .filter(|row| row.enabled && row.last_test_ok == Some(true))
                .map(|row| toolbox::managed_query_name(row.id))
                .collect();
            wait_for_toolbox(&expected, false).await
        }
        .await;
        if let Err(error) = publish {
            self.mark_enabled_database_error(pool, &error).await?;
            return match toolbox::reset().await {
                Ok(()) => Err(error),
                Err(reset_error) => Err(format!(
                    "{error}; tampoco se pudo retirar configuración anterior: {reset_error}"
                )),
            };
        }
        Ok(())
    }

    async fn mark_enabled_database_error(&self, pool: &PgPool, error: &str) -> Result<(), String> {
        for row in db::list_database_connections(pool)
            .await
            .map_err(|err| err.to_string())?
            .into_iter()
            .filter(|row| row.enabled)
        {
            db::set_database_test_result(pool, row.id, false, Some(error))
                .await
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub async fn begin_codex_login(&self, provider_id: Uuid) -> Result<CodexLoginDto, String> {
        let pool = self.pool().await?;
        let snap = db::load(&pool).await.map_err(|e| e.to_string())?;
        let provider = snap
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .ok_or_else(|| "proveedor inexistente".to_string())?;
        if !provider.kind.eq_ignore_ascii_case("codex") {
            return Err("el proveedor no es Codex".into());
        }

        let login = OAuthClient::new()
            .begin_device_login()
            .await
            .map_err(|e| e.to_string())?;
        let id = Uuid::new_v4();
        let out = CodexLoginDto {
            id,
            verification_url: login.verification_url.clone(),
            user_code: login.authorization.user_code.clone(),
        };
        self.inner
            .codex_logins
            .lock()
            .await
            .insert(id, PendingCodexLogin { provider_id, login });
        Ok(out)
    }

    pub async fn finish_codex_login(&self, id: Uuid) -> Result<SnapshotDto, String> {
        let pending = self
            .inner
            .codex_logins
            .lock()
            .await
            .get(&id)
            .cloned()
            .ok_or_else(|| "login Codex inexistente o caducado".to_string())?;

        let result = tokio::time::timeout(
            Duration::from_secs(15 * 60),
            OAuthClient::new().finish_device_login(&pending.login),
        )
        .await;
        self.inner.codex_logins.lock().await.remove(&id);
        let credentials = result
            .map_err(|_| "el login Codex ha caducado".to_string())?
            .map_err(|e| e.to_string())?;
        let pool = self.pool().await?;
        PostgresCodexTokenStore::new(pool.clone(), pending.provider_id)
            .save(&credentials)
            .await
            .map_err(|e| e.to_string())?;
        db::sync_codex_provider(&pool, pending.provider_id).await?;
        let snap = db::load(&pool).await.map_err(|e| e.to_string())?;
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
            ira_llm::LlmError::MissingKey(var) => {
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
        let mut result = ira_tools::chat(&client, req.clone(), &registry).await;
        if let Err(err) = &result
            && !registry.is_empty()
            && tools_unsupported(err)
        {
            result = ira_tools::chat(&client, req, &Registry::default()).await;
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

fn validate_database_input(
    mut input: DatabaseInput,
    require_password: bool,
) -> Result<DatabaseInput, String> {
    input.name = input.name.trim().to_string();
    input.host = input.host.trim().to_string();
    input.database = input.database.trim().to_string();
    input.username = input.username.trim().to_string();
    input.password = input.password.filter(|password| !password.is_empty());
    input.ssl_mode = input.ssl_mode.trim().to_ascii_lowercase();
    if input.name.is_empty()
        || input.host.is_empty()
        || input.database.is_empty()
        || input.username.is_empty()
    {
        return Err("nombre, host, base de datos y usuario son obligatorios".into());
    }
    if !(1..=65535).contains(&input.port) {
        return Err("puerto fuera de rango (1-65535)".into());
    }
    if !matches!(
        input.ssl_mode.as_str(),
        "disable" | "prefer" | "require" | "verify-ca" | "verify-full"
    ) {
        return Err("modo SSL inválido".into());
    }
    if require_password && input.password.is_none() {
        return Err("contraseña obligatoria".into());
    }
    Ok(input)
}

fn database_host(host: &str) -> &str {
    if host.eq_ignore_ascii_case("host.docker.internal") {
        "127.0.0.1"
    } else {
        host
    }
}

fn filled(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn database_write(input: DatabaseInput) -> db::DatabaseWrite {
    db::DatabaseWrite {
        name: input.name,
        host: input.host,
        port: input.port,
        database: input.database,
        username: input.username,
        password: input.password,
        ssl_mode: input.ssl_mode,
        enabled: input.enabled,
    }
}

async fn test_postgres(row: &db::DatabaseConnectionRow, password: &str) -> Result<bool, String> {
    let ssl_mode = match row.ssl_mode.as_str() {
        "disable" => PgSslMode::Disable,
        "prefer" => PgSslMode::Prefer,
        "require" => PgSslMode::Require,
        "verify-ca" => PgSslMode::VerifyCa,
        "verify-full" => PgSslMode::VerifyFull,
        _ => return Err("modo SSL inválido".into()),
    };
    let options = PgConnectOptions::new()
        .host(database_host(&row.host))
        .port(row.port as u16)
        .database(&row.database)
        .username(&row.username)
        .password(password)
        .ssl_mode(ssl_mode)
        .application_name("ira-ai-test");
    let connect = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options);
    let remote = tokio::time::timeout(Duration::from_secs(12), connect)
        .await
        .map_err(|_| "la conexión superó 12 segundos".to_string())?
        .map_err(|err| format!("no se pudo conectar: {err}"))?;
    let checked = tokio::time::timeout(Duration::from_secs(5), async {
        sqlx::query_scalar::<_, String>("SHOW transaction_read_only")
            .fetch_one(&remote)
            .await
    })
    .await;
    remote.close().await;
    let mode = checked
        .map_err(|_| "conectó, pero la prueba superó 5 segundos".to_string())?
        .map_err(|err| format!("conectó, pero falló la prueba: {err}"))?;
    Ok(mode.eq_ignore_ascii_case("on"))
}

async fn wait_for_toolbox(expected: &[String], require_empty: bool) -> Result<(), String> {
    let client = ira_tools_db::Client::from_env()
        .ok_or_else(|| "falta MCP_TOOLBOX_URL para publicar conexiones".to_string())?;
    let mut last_error = None;
    for _ in 0..80 {
        match client.list_tools().await {
            Ok(tools) => {
                let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
                let ready = if require_empty {
                    names.iter().all(|name| !name.starts_with("db_"))
                } else {
                    expected.iter().all(|name| names.contains(&name.as_str()))
                };
                if ready {
                    return Ok(());
                }
            }
            Err(error) => last_error = Some(error.to_string()),
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(last_error.unwrap_or_else(|| {
        if require_empty {
            "Toolbox no retiró la configuración anterior".into()
        } else {
            "Toolbox no pudo activar una o más conexiones".into()
        }
    }))
}

fn tools_unsupported(err: &ira_llm::LlmError) -> bool {
    let text = err.to_string().to_ascii_lowercase();
    text.contains("does not support tools") || text.contains("does not support tool")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ira_store::ProviderRow;

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
        let snap = ira_store::stub_snapshot("ollama", "gemma3:latest", "hola");
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
        let err = ira_llm::LlmError::Http {
            status: 400,
            body: "registry.ollama.ai/library/gemma3:4b does not support tools".into(),
        };
        assert!(tools_unsupported(&err));
        assert!(!tools_unsupported(&ira_llm::LlmError::Empty("modelo")));
    }

    fn database_input() -> DatabaseInput {
        DatabaseInput {
            name: " Analytics ".into(),
            host: " db.internal ".into(),
            port: 5432,
            database: " reports ".into(),
            username: " reader ".into(),
            password: Some("secret".into()),
            ssl_mode: " REQUIRE ".into(),
            enabled: true,
        }
    }

    #[test]
    fn validates_and_normalizes_database_input() {
        let input = validate_database_input(database_input(), true).expect("valid input");
        assert_eq!(input.name, "Analytics");
        assert_eq!(input.host, "db.internal");
        assert_eq!(input.database, "reports");
        assert_eq!(input.username, "reader");
        assert_eq!(input.ssl_mode, "require");
    }

    #[test]
    fn rejects_unsafe_database_shape() {
        let mut input = database_input();
        input.port = 0;
        assert!(validate_database_input(input, true).is_err());

        let mut input = database_input();
        input.ssl_mode = "trust-everything".into();
        assert!(validate_database_input(input, true).is_err());

        let mut input = database_input();
        input.password = None;
        assert!(validate_database_input(input, true).is_err());
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
