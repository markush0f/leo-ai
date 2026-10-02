//! Shared chat and catalog service for the desktop bridge and `ira-server`.
//!
//! [`App`] talks to PostgreSQL and the active provider (including Ollama) through
//! `ira-store`, `ira-llm`, and `ira-tools`. [`App::start_services`] brings up
//! the local Compose stack (Postgres and MCP Toolbox). Frontend DTOs expose
//! credential availability, never stored secrets.

mod dto;
mod host;
mod models_dev;
mod preferences;
mod toolbox;

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use ira_llm::ChatRequest;
use ira_llm::providers::codex::{DeviceLogin, OAuthClient, TokenStore};
use ira_store::{self as db, CHANNEL_LOCAL, CONTEXT_LIMIT, NewMessage, PostgresCodexTokenStore};
use ira_tools::Registry;
use serde::{Deserialize, Serialize};
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
pub use preferences::AssistantPreferences;

/// Event produced while a chat turn is streamed to a transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatStreamEvent {
    Delta {
        text: String,
    },
    Reset,
    McpUsed {
        server_id: String,
        tool_name: String,
    },
    Done,
    Error { error: String },
}

/// Transport callback for [`App::chat_stream`].
pub type ChatStreamSink = Arc<dyn Fn(ChatStreamEvent) + Send + Sync + 'static>;

#[derive(Debug, Clone, Deserialize)]
pub struct McpInput {
    pub name: String,
    pub transport: String,
    pub url: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default = "mcp_enabled")]
    pub enabled: bool,
}

fn mcp_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct McpView {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub url: Option<String>,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub enabled: bool,
    pub editable: bool,
}

impl McpView {
    fn from_config(config: ira_mcp::McpServerConfig, editable: bool) -> Self {
        // Keep literal secrets on disk. Environment references are safe to edit in the UI.
        let redact = |values: HashMap<String, String>| {
            values
                .into_iter()
                .map(|(key, value)| {
                    (
                        key,
                        if value.contains("${SECRET:") || value.contains("{env:") {
                            value
                        } else {
                            String::new()
                        },
                    )
                })
                .collect()
        };
        let bridge = config.transport == ira_mcp::McpTransport::Stdio
            && config.command.as_deref() == Some("npx")
            && config.args.get(1).is_some_and(|arg| arg == "mcp-remote");
        Self {
            id: config.id,
            name: config.name,
            transport: if bridge {
                "remote_bridge"
            } else {
                config.transport.as_str()
            }
            .into(),
            url: if bridge {
                config.args.get(2).cloned()
            } else {
                config.url
            },
            command: if bridge { None } else { config.command },
            args: if bridge { Vec::new() } else { config.args },
            env: redact(config.env),
            headers: redact(config.headers),
            enabled: config.enabled,
            editable,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct McpTest {
    pub tools: Vec<String>,
}

#[derive(Clone)]
pub struct App {
    inner: Arc<Inner>,
}

struct Inner {
    pool: RwLock<Option<PgPool>>,
    db_error: RwLock<Option<String>>,
    tools: Registry,
    toolbox_sync: Mutex<()>,
    mcp_config: Mutex<()>,
    codex_logins: Mutex<HashMap<Uuid, PendingCodexLogin>>,
    conversation_locks: Mutex<HashMap<Uuid, Weak<Mutex<()>>>>,
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
                mcp_config: Mutex::new(()),
                codex_logins: Mutex::new(HashMap::new()),
                conversation_locks: Mutex::new(HashMap::new()),
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
            let mut out = self.host_status().await;
            out.ok = false;
            out.error = Some(error);
            return out;
        }
        let catalog = self.service_catalog().await;
        let mut out = host::start(&catalog).await;
        if out.error.is_some()
            || !out
                .services
                .iter()
                .any(|service| service.id == "postgres" && service.autostart)
        {
            return out;
        }
        for _ in 0..25 {
            if self.try_connect().await.is_ok() {
                return self.host_status().await;
            }
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
        let err = self.try_connect().await.err();
        out = self.host_status().await;
        if let Some(err) = err {
            out.ok = false;
            out.error = Some(err);
        }
        out
    }

    pub async fn services(&self) -> ServicesDto {
        self.host_status().await
    }

    /// Starts the services marked for boot. Safe to call from process startup.
    pub async fn boot_services(&self) -> ServicesDto {
        let catalog = self.service_catalog().await;
        let out = host::start(&catalog).await;
        if out.error.is_none()
            && out
                .services
                .iter()
                .any(|service| service.id == "postgres" && service.autostart)
        {
            let _ = self.try_connect().await;
        }
        self.host_status().await
    }

    /// Starts, stops, or reconfigures one Compose service. Starting Postgres also reconnects.
    pub async fn set_service(
        &self,
        id: &str,
        action: &str,
        port: Option<u16>,
        name: Option<&str>,
        description: Option<&str>,
    ) -> ServicesDto {
        let catalog = self.service_catalog().await;
        let out = match action {
            "start" => host::start_one(id, &catalog).await,
            "stop" => host::stop_one(id, &catalog).await,
            "autostart" | "manual" => match host::set_boot(id, action == "autostart", &catalog) {
                Ok(()) => host::status(&catalog).await,
                Err(error) => {
                    let mut out = host::status(&catalog).await;
                    out.ok = false;
                    out.error = Some(error);
                    out
                }
            },
            "meta" => {
                match host::set_meta(id, name.unwrap_or(""), description.unwrap_or(""), &catalog) {
                    Ok(()) => host::status(&catalog).await,
                    Err(error) => {
                        let mut out = host::status(&catalog).await;
                        out.ok = false;
                        out.error = Some(error);
                        out
                    }
                }
            }
            "port" => {
                let Some(port) = port else {
                    let mut out = host::status(&catalog).await;
                    out.ok = false;
                    out.error = Some("falta el puerto".into());
                    return out;
                };
                host::set_port(id, port, &catalog).await
            }
            _ => {
                let mut out = host::status(&catalog).await;
                out.ok = false;
                out.error = Some("acción inválida".into());
                return out;
            }
        };
        if (action == "start" || action == "port") && id == "postgres" && out.error.is_none() {
            for _ in 0..25 {
                if self.try_connect().await.is_ok() {
                    return self.host_status().await;
                }
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            }
            let err = self.try_connect().await.err();
            let mut out = self.host_status().await;
            if let Some(err) = err {
                out.ok = false;
                out.error = Some(err);
            }
            return out;
        }
        out
    }

    async fn service_catalog(&self) -> Vec<host::CatalogEntry> {
        let pool = self.inner.pool.read().await.clone();
        if let Some(pool) = &pool
            && let Ok(found) = host::discover_catalog().await
            && !found.is_empty()
        {
            let rows: Vec<db::HostServiceRow> = found
                .iter()
                .enumerate()
                .map(|(index, entry)| db::HostServiceRow {
                    id: entry.id.clone(),
                    name: entry.name.clone(),
                    required: entry.required,
                    position: index as i32,
                })
                .collect();
            let _ = db::sync_host_services(pool, &rows).await;
            return found;
        }
        if let Some(pool) = pool
            && let Ok(rows) = db::list_host_services(&pool).await
            && !rows.is_empty()
        {
            return rows
                .into_iter()
                .map(|row| {
                    let kind = if row.id.ends_with("-mcp") {
                        "mcp"
                    } else {
                        "service"
                    };
                    host::CatalogEntry {
                        id: row.id,
                        name: row.name,
                        required: row.required,
                        host_port: None,
                        container_port: None,
                        kind: kind.into(),
                        description: String::new(),
                        peer: None,
                    }
                })
                .collect();
        }
        host::builtin_catalog()
    }

    async fn system_text(&self, base: &str, model: &str) -> String {
        let note = host::catalog_note(&self.service_catalog().await);
        let model_context = format!(
            "El modelo de IA que estás usando actualmente es \"{model}\". Si te preguntan qué modelo eres o cuál estás usando, responde de forma natural y honesta con este nombre. No digas que eres el modelo; explica que es el modelo que te impulsa."
        );
        let tools_context = "Usa las herramientas disponibles cuando sean útiles para responder, incluidas herramientas MCP locales y externas. La lista de servicios locales no representa todos los MCP conectados: comprueba las herramientas disponibles en esta conversación y úsalas según su descripción. Para resultados deportivos actuales, consulta herramientas deportivas disponibles antes de responder. No afirmes que un MCP no está conectado si no lo verificaste intentando usar sus herramientas; si no hay herramienta pertinente o falla, explica esa limitación concreta.";
        let web_search_context = "Cuando el usuario pida buscar en internet, consultar la web o información reciente, usa la herramienta web_search si está disponible. Para noticias o datos que cambian, prioriza páginas recién publicadas o actualizadas, comprueba su fecha y distingue fecha de publicación de fecha del evento. Si las fuentes no son suficientemente recientes, dilo claramente en vez de presentar datos antiguos como actuales. Haz la búsqueda en segundo plano: no abras ni controles el navegador local del usuario. Después, responde en este mismo turno con un resumen útil y menciona fuentes cuando estén disponibles; no te limites a iniciar una búsqueda ni dejes al usuario esperando.";
        [base, &model_context, tools_context, web_search_context, &note]
            .into_iter()
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    async fn host_status(&self) -> ServicesDto {
        host::status(&self.service_catalog().await).await
    }

    async fn tools(&self) -> Registry {
        let mut configs = match ira_mcp::load_all() {
            Ok(configs) => configs,
            Err(error) => {
                tracing::warn!(%error, "mcp config");
                Vec::new()
            }
        };
        let discovered = host::discover_mcp().await.unwrap_or_default();
        for endpoint in discovered {
            if configs
                .iter()
                .any(|server| server.id == endpoint.id || server.name == endpoint.id)
            {
                continue;
            }
            configs.push(db::McpServerConfig::http(
                endpoint.id.clone(),
                ira_mcp::mcp_endpoint(&endpoint.url),
            ));
        }
        ira_tools::attach_configured(self.inner.tools.clone(), &configs).await
    }

    pub async fn list_mcp(&self) -> Result<Vec<McpView>, String> {
        let _guard = self.inner.mcp_config.lock().await;
        let global = ira_mcp::read_servers(&ira_mcp::global_path()).map_err(|e| e.to_string())?;
        let mut views = Vec::new();
        for config in ira_mcp::load_all().map_err(|e| e.to_string())? {
            let editable = global.iter().any(|entry| entry.id == config.id)
                && !ira_mcp::project_file().is_some_and(|path| {
                    ira_mcp::read_servers(&path)
                        .is_ok_and(|rows| rows.iter().any(|entry| entry.id == config.id))
                });
            views.push(McpView::from_config(config, editable));
        }
        Ok(views)
    }

    pub async fn save_mcp(&self, input: McpInput) -> Result<McpView, String> {
        let _guard = self.inner.mcp_config.lock().await;
        let name = input.name.trim();
        let id = ira_mcp::slug(name);
        if name.is_empty() || id.is_empty() || id.len() > 64 {
            return Err("nombre MCP inválido".into());
        }
        if ira_mcp::project_file().is_some_and(|path| {
            ira_mcp::read_servers(&path).is_ok_and(|rows| rows.iter().any(|entry| entry.id == id))
        }) {
            return Err("este MCP pertenece al proyecto; edita su ira.json".into());
        }
        let previous = ira_mcp::read_servers(&ira_mcp::global_path())
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|entry| entry.id == id);
        let mut config = match input.transport.as_str() {
            "remote_bridge" => {
                let url = input.url.unwrap_or_default();
                if !url.starts_with("https://") && !url.starts_with("http://") {
                    return Err("URL MCP inválida".into());
                }
                let allow_http = url.starts_with("http://");
                let mut args = vec![
                    "-y".into(),
                    "mcp-remote".into(),
                    url,
                    "--protocol".into(),
                    "auto".into(),
                    "--auth-timeout".into(),
                    "120".into(),
                ];
                if allow_http {
                    args.push("--allow-http".into());
                }
                ira_mcp::McpServerConfig::stdio(id, name, "npx", args, input.env)
            }
            "stdio" => {
                let command = input.command.unwrap_or_default();
                ira_mcp::McpServerConfig::stdio(id, name, command, input.args, input.env)
            }
            "streamable_http" => {
                let url = input.url.unwrap_or_default();
                if !url.starts_with("https://") && !url.starts_with("http://") {
                    return Err("URL MCP inválida".into());
                }
                let mut config = ira_mcp::McpServerConfig::http(id, url);
                config.name = name.to_string();
                config.headers = input.headers;
                config
            }
            _ => return Err("transporte MCP desconocido".into()),
        };
        if let Some(previous) = previous.filter(|entry| entry.transport == config.transport) {
            for (key, value) in &mut config.env {
                if value.is_empty() {
                    *value = previous.env.get(key).cloned().unwrap_or_default();
                }
            }
            for (key, value) in &mut config.headers {
                if value.is_empty() {
                    *value = previous.headers.get(key).cloned().unwrap_or_default();
                }
            }
        }
        config.enabled = input.enabled;
        config.validate()?;
        ira_mcp::upsert(ira_mcp::Scope::Global, &config).map_err(|e| e.to_string())?;
        ira_mcp::shared().disconnect(&config.id).await;
        Ok(McpView::from_config(config, true))
    }

    pub async fn delete_mcp(&self, id: &str) -> Result<DeleteDto, String> {
        let _guard = self.inner.mcp_config.lock().await;
        ira_mcp::remove_global(id).map_err(|e| e.to_string())?;
        ira_mcp::shared().disconnect(id).await;
        Ok(DeleteDto { ok: true })
    }

    pub async fn test_mcp(&self, id: &str) -> Result<McpTest, String> {
        let config = ira_mcp::load_all()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|config| config.id == id)
            .ok_or_else(|| "MCP desconocido".to_string())?;
        let manager = ira_mcp::shared();
        manager.disconnect(id).await;
        manager.connect(&config).await.map_err(|e| e.to_string())?;
        let tools = manager.list_tools(id).await.map_err(|e| e.to_string())?;
        Ok(McpTest {
            tools: tools.into_iter().map(|tool| tool.name).collect(),
        })
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

    async fn conversation_lock(&self, id: Uuid) -> Arc<Mutex<()>> {
        let mut locks = self.inner.conversation_locks.lock().await;
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(&id).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(Mutex::new(()));
        locks.insert(id, Arc::downgrade(&lock));
        lock
    }

    pub async fn snapshot(&self) -> Result<SnapshotDto, String> {
        let pool = self.pool().await?;
        let mut snap = db::load(&pool).await.map_err(|e| e.to_string())?;
        let stored_preferences = preferences::load()?;
        let catalog = models_dev::load().await?;
        let (preferences, model_dtos) = apply_models_dev(&mut snap, stored_preferences, &catalog);
        if preferences != preferences::load()? {
            preferences::save(&preferences)?;
        }
        let mut snapshot = dto::snapshot_dto(snap, &self.inner.tools.names());
        snapshot.models = model_dtos;
        snapshot.web_search_enabled = preferences.web_search_enabled;
        snapshot.web_search_context_size = preferences.web_search_context_size;
        Ok(snapshot)
    }

    pub fn update_web_search(
        &self,
        enabled: bool,
        context_size: &str,
    ) -> Result<AssistantPreferences, String> {
        preferences::save_web_search(enabled, context_size)
    }

    pub async fn apply(&self, op: Op) -> Result<SnapshotDto, String> {
        let pool = self.pool().await?;
        if let Some(snapshot) = self.apply_model_preference(&pool, &op).await? {
            return Ok(snapshot);
        }
        let mut snap = db::apply(&pool, op.clone().into())
            .await
            .map_err(|e| e.to_string())?;
        let mut preferences = preferences::load()?;
        let persist_preferences = match op {
            Op::SetSystem { text } => {
                preferences.system_prompt = Some(text);
                true
            }
            Op::ActivateModel { .. } | Op::ActivateProvider { .. } => {
                if let (Some(provider), Some(model)) = (snap.active_provider(), snap.active_model())
                {
                    preferences.active_provider = Some(provider.kind.clone());
                    preferences.active_provider_id = Some(provider.id.to_string());
                    preferences.active_model = Some(model.name.clone());
                }
                true
            }
            _ => false,
        };
        if persist_preferences {
            preferences::save(&preferences)?;
        }
        let catalog = models_dev::load().await?;
        let (preferences, model_dtos) = apply_models_dev(&mut snap, preferences, &catalog);
        let mut snapshot = dto::snapshot_dto(snap, &self.inner.tools.names());
        snapshot.models = model_dtos;
        snapshot.web_search_enabled = preferences.web_search_enabled;
        snapshot.web_search_context_size = preferences.web_search_context_size;
        Ok(snapshot)
    }

    async fn apply_model_preference(
        &self,
        pool: &PgPool,
        op: &Op,
    ) -> Result<Option<SnapshotDto>, String> {
        if !matches!(
            op,
            Op::ActivateModel { .. } | Op::ActivateProvider { .. } | Op::SetModelEffort { .. }
        ) {
            return Ok(None);
        }
        let mut snap = db::load(pool).await.map_err(|error| error.to_string())?;
        let stored = preferences::load()?;
        let catalog = models_dev::load().await?;
        let (mut preferences, models) = apply_models_dev(&mut snap, stored, &catalog);
        let selected = match op {
            Op::ActivateModel { id } => models.iter().find(|model| model.id == *id),
            Op::SetModelEffort { id, .. } => models.iter().find(|model| model.id == *id),
            Op::ActivateProvider { id } => {
                let provider = snap.providers.iter().find(|provider| provider.id == *id);
                let Some(provider) = provider else {
                    return Ok(None);
                };
                let first = models.iter().find(|model| model.provider_id == provider.id);
                if let Some(model) = first {
                    preferences.active_provider = Some(provider.kind.clone());
                    preferences.active_provider_id = Some(provider.id.to_string());
                    preferences.active_model = Some(model.name.clone());
                    preferences.active_effort = model.effort_options.first().cloned();
                    preferences::save(&preferences)?;
                    return Ok(Some(self.snapshot().await?));
                }
                return Ok(None);
            }
            _ => return Ok(None),
        };
        let Some(model) = selected else {
            return Ok(None);
        };
        let provider = snap
            .providers
            .iter()
            .find(|provider| provider.id == model.provider_id)
            .ok_or_else(|| "proveedor del modelo no encontrado".to_string())?;
        preferences.active_provider = Some(provider.kind.clone());
        preferences.active_provider_id = Some(provider.id.to_string());
        preferences.active_model = Some(model.name.clone());
        match op {
            Op::SetModelEffort { effort, .. } => {
                if !model.effort_options.iter().any(|value| value == effort) {
                    return Err("potencia no compatible con este modelo".into());
                }
                preferences.active_effort = Some(effort.clone());
            }
            _ => preferences.active_effort = model.effort_options.first().cloned(),
        }
        preferences::save(&preferences)?;
        Ok(Some(self.snapshot().await?))
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
        let mut preferences = preferences::load()?;
        preferences.active_provider = Some("codex".into());
        preferences.active_provider_id = Some(pending.provider_id.to_string());
        preferences::save(&preferences)?;
        self.snapshot().await
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

    pub async fn rename_chat(
        &self,
        id: Uuid,
        title: String,
    ) -> Result<Vec<ConversationDto>, String> {
        let title = title.trim();
        if title.is_empty() {
            return Err("el nombre no puede estar vacío".into());
        }
        let pool = self.pool().await?;
        db::rename_conversation(&pool, id, title)
            .await
            .map_err(|e| e.to_string())?;
        self.list_chats().await
    }

    pub async fn delete_chat(&self, id: Uuid) -> Result<Vec<ConversationDto>, String> {
        let pool = self.pool().await?;
        db::archive_conversation(&pool, id)
            .await
            .map_err(|e| e.to_string())?;
        let chats = self.list_chats().await?;
        if chats.is_empty() {
            self.new_chat().await?;
        } else {
            db::set_active_conversation(&pool, Some(chats[0].id))
                .await
                .map_err(|e| e.to_string())?;
        }
        self.list_chats().await
    }

    pub async fn delete_all_chats(&self) -> Result<Vec<ConversationDto>, String> {
        let pool = self.pool().await?;
        db::archive_local_conversations(&pool)
            .await
            .map_err(|e| e.to_string())?;
        self.new_chat().await?;
        self.list_chats().await
    }

    pub async fn chat(&self, conversation_id: Uuid, text: String) -> Result<ChatOut, String> {
        let conversation_lock = self.conversation_lock(conversation_id).await;
        let _turn = conversation_lock.lock().await;
        let pool = self.pool().await?;
        let mut snap = db::load(&pool).await.map_err(|e| e.to_string())?;
        let preferences = preferences::load()?;
        let catalog = models_dev::load().await?;
        let (preferences, _) = apply_models_dev(&mut snap, preferences, &catalog);
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
        let model = snap
            .active_model()
            .map_or("desconocido", |model| model.name.as_str());
        let mut req =
            ChatRequest::with_history(&self.system_text(&snap.system, model).await, history);
        if preferences.web_search_enabled
            && snap
                .active_provider()
                .is_some_and(|provider| provider.kind.eq_ignore_ascii_case("codex"))
        {
            req.tools.push(ira_llm::ToolSpec {
                name: "web_search".into(),
                description:
                    "Busca en internet cuando el usuario solicite información web o actualizada."
                        .into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "search_context_size": preferences.web_search_context_size,
                }),
            });
        }
        req.reasoning_effort = preferences.active_effort.clone();
        let registry = chat_tools(self.tools().await, &snap);
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
                    // models.dev IDs are virtual and are not rows in the database `models` table.
                    NewMessage::assistant(resp.text.clone(), None),
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

    pub async fn chat_stream(&self, conversation_id: Uuid, text: String, sink: ChatStreamSink) {
        let conversation_lock = self.conversation_lock(conversation_id).await;
        let _turn = conversation_lock.lock().await;
        let pool = match self.pool().await {
            Ok(pool) => pool,
            Err(error) => {
                sink(ChatStreamEvent::Error { error });
                return;
            }
        };
        let mut snap = match db::load(&pool).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                sink(ChatStreamEvent::Error {
                    error: error.to_string(),
                });
                return;
            }
        };
        let preferences = match preferences::load() {
            Ok(preferences) => preferences,
            Err(error) => {
                sink(ChatStreamEvent::Error { error });
                return;
            }
        };
        let catalog = match models_dev::load().await {
            Ok(catalog) => catalog,
            Err(error) => {
                sink(ChatStreamEvent::Error { error });
                return;
            }
        };
        let (preferences, _) = apply_models_dev(&mut snap, preferences, &catalog);
        let client = match db::client_with_pool(&snap, &pool) {
            Ok(client) => client,
            Err(error) => {
                let error = match error {
                    ira_llm::LlmError::MissingKey(var) => {
                        format!("falta api key ({var}): ábrelo en catálogo")
                    }
                    other => other.to_string(),
                };
                sink(ChatStreamEvent::Error { error });
                return;
            }
        };
        if let Err(error) = db::append_message(&pool, conversation_id, NewMessage::user(text)).await
        {
            sink(ChatStreamEvent::Error {
                error: error.to_string(),
            });
            return;
        }

        let result = async {
            let history = db::context_messages(&pool, conversation_id, CONTEXT_LIMIT)
                .await
                .map_err(|error| error.to_string())?;
            let model = snap
                .active_model()
                .map_or("desconocido", |model| model.name.as_str());
            let mut req =
                ChatRequest::with_history(&self.system_text(&snap.system, model).await, history);
            if preferences.web_search_enabled
                && snap
                .active_provider()
                .is_some_and(|provider| provider.kind.eq_ignore_ascii_case("codex"))
            {
                req.tools.push(ira_llm::ToolSpec {
                    name: "web_search".into(),
                    description: "Busca en internet cuando el usuario solicite información web o actualizada.".into(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {},
                        "search_context_size": preferences.web_search_context_size,
                    }),
                });
            }
            req.reasoning_effort = preferences.active_effort.clone();
            let registry = chat_tools(self.tools().await, &snap);
            let event_sink = sink.clone();
            let tool_sink: ira_tools::StreamSink = Arc::new(move |event| match event {
                ira_tools::StreamEvent::Delta(text) => event_sink(ChatStreamEvent::Delta { text }),
                ira_tools::StreamEvent::Reset => event_sink(ChatStreamEvent::Reset),
                ira_tools::StreamEvent::McpUsed { server_id, tool_name } => {
                    event_sink(ChatStreamEvent::McpUsed { server_id, tool_name })
                }
                ira_tools::StreamEvent::Memory { .. } => {}
            });
            let mut response =
                ira_tools::chat_stream(&client, req.clone(), &registry, tool_sink.clone()).await;
            if let Err(error) = &response
                && !registry.is_empty()
                && tools_unsupported(error)
            {
                sink(ChatStreamEvent::Reset);
                response =
                    ira_tools::chat_stream(&client, req, &Registry::default(), tool_sink).await;
            }
            response.map_err(|error| error.to_string())
        }
        .await;

        match result {
            Ok(response) => {
                if let Err(error) = db::append_message(
                    &pool,
                    conversation_id,
                    // models.dev IDs are virtual and are not rows in the database `models` table.
                    NewMessage::assistant(response.text, None),
                )
                .await
                {
                    let error = error.to_string();
                    let _ = db::append_message(
                        &pool,
                        conversation_id,
                        NewMessage::error(error.clone()),
                    )
                    .await;
                    sink(ChatStreamEvent::Error { error });
                    return;
                }
                sink(ChatStreamEvent::Done);
            }
            Err(error) => {
                let _ =
                    db::append_message(&pool, conversation_id, NewMessage::error(error.clone()))
                        .await;
                sink(ChatStreamEvent::Error { error });
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

fn chat_tools(registry: Registry, snap: &ira_store::Snapshot) -> Registry {
    if !snap.settings.tools_enabled {
        return Registry::default();
    }
    if snap.settings.tools_mutate {
        registry
    } else {
        registry.read_only()
    }
}

fn apply_models_dev(
    snap: &mut ira_store::Snapshot,
    mut preferences: AssistantPreferences,
    catalog: &[models_dev::CatalogModel],
) -> (AssistantPreferences, Vec<ModelDto>) {
    let fallback = snap.active_model().and_then(|model| {
        snap.providers
            .iter()
            .find(|provider| provider.id == model.provider_id)
            .map(|provider| (Some(provider.id), provider.kind.clone(), model.name.clone()))
    });
    let mut candidates = preferences
        .active_provider
        .clone()
        .zip(preferences.active_model.clone())
        .into_iter()
        .map(|(kind, name)| {
            (
                preferences
                    .active_provider_id
                    .as_deref()
                    .and_then(|id| Uuid::parse_str(id).ok()),
                kind,
                name,
            )
        })
        .collect::<Vec<_>>();
    if let Some(fallback) = fallback
        && !candidates.contains(&fallback)
    {
        candidates.push(fallback);
    }
    let chosen = candidates
        .into_iter()
        .find_map(|(preferred_id, kind, name)| {
            let providers = snap
                .providers
                .iter()
                .filter(|provider| provider.kind.eq_ignore_ascii_case(&kind));
            let provider = preferred_id
                .and_then(|id| providers.clone().find(|provider| provider.id == id))
                .or_else(|| {
                    let matching = snap
                        .providers
                        .iter()
                        .filter(|provider| provider.kind.eq_ignore_ascii_case(&kind));
                    if kind.eq_ignore_ascii_case("codex") {
                        matching
                            .clone()
                            .find(|provider| {
                                provider
                                    .api_key
                                    .as_deref()
                                    .is_some_and(|key| !key.trim().is_empty())
                            })
                            .or_else(|| matching.into_iter().next())
                    } else {
                        matching.into_iter().next()
                    }
                })?;
            let source = models_dev::provider_id(&provider.kind)?;
            models_dev::models_for(catalog, source)
                .into_iter()
                .find(|model| model.id == name)
                .map(|model| (provider.id, provider.kind.clone(), model))
        });
    let chosen = chosen.or_else(|| {
        snap.providers.iter().find_map(|provider| {
            let source = models_dev::provider_id(&provider.kind)?;
            models_dev::models_for(catalog, source)
                .into_iter()
                .next()
                .map(|model| (provider.id, provider.kind.clone(), model))
        })
    });

    let Some((provider_id, provider_kind, active)) = chosen else {
        snap.models.clear();
        snap.active_model_id = None;
        snap.settings.active_model_id = None;
        if let Some(system_prompt) = &preferences.system_prompt {
            snap.system.clone_from(system_prompt);
            snap.settings.system_prompt.clone_from(system_prompt);
        }
        return (preferences, Vec::new());
    };

    preferences.active_provider = Some(provider_kind);
    preferences.active_provider_id = Some(provider_id.to_string());
    preferences.active_model = Some(active.id.clone());
    if !preferences
        .active_effort
        .as_ref()
        .is_some_and(|effort| active.reasoning_options.contains(effort))
    {
        preferences.active_effort = active.reasoning_options.first().cloned();
    }
    let active_effort = preferences.active_effort.clone().unwrap_or_default();
    let mut models = Vec::new();
    for provider in &snap.providers {
        let Some(source) = models_dev::provider_id(&provider.kind) else {
            continue;
        };
        for model in models_dev::models_for(catalog, source) {
            let id = Uuid::new_v5(&provider.id, model.id.as_bytes());
            let effort = if provider.id == provider_id && model.id == active.id {
                active_effort.clone()
            } else {
                model.reasoning_options.first().cloned().unwrap_or_default()
            };
            models.push(ira_store::ModelRow {
                id,
                provider_id: provider.id,
                name: model.id.clone(),
                effort,
            });
        }
    }
    let active_id = Uuid::new_v5(&provider_id, active.id.as_bytes());
    snap.models = models;
    snap.active_model_id = Some(active_id);
    snap.settings.active_model_id = Some(active_id);
    if let Some(system_prompt) = &preferences.system_prompt {
        snap.system.clone_from(system_prompt);
        snap.settings.system_prompt.clone_from(system_prompt);
    }

    let model_dtos = snap
        .providers
        .iter()
        .flat_map(|provider| {
            let Some(source) = models_dev::provider_id(&provider.kind) else {
                return Vec::new();
            };
            models_dev::models_for(catalog, source)
                .into_iter()
                .map(|model| {
                    let id = Uuid::new_v5(&provider.id, model.id.as_bytes());
                    let effort = snap
                        .models
                        .iter()
                        .find(|row| row.id == id)
                        .map(|row| row.effort.clone())
                        .unwrap_or_default();
                    ModelDto {
                        id,
                        provider_id: provider.id,
                        name: model.id.clone(),
                        display_name: model.name.clone(),
                        effort,
                        effort_options: model.reasoning_options.clone(),
                        reasoning: model.reasoning,
                        context_window: model.context_window,
                        output_limit: model.output_limit,
                        release_date: model.release_date.clone(),
                        last_updated: model.last_updated.clone(),
                    }
                })
                .collect()
        })
        .collect();
    (preferences, model_dtos)
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
        assert_eq!(json["tools_mutate"], false);
        assert_eq!(json["active_model_id"].as_str().unwrap().len(), 36);
    }

    #[test]
    fn models_dev_catalog_sets_active_model_and_power_without_database_models() {
        let mut snap = ira_store::stub_snapshot("codex", "old-model", "prompt");
        let catalog = vec![models_dev::CatalogModel {
            provider: "openai".into(),
            id: "gpt-6-luna".into(),
            name: "GPT-6 Luna".into(),
            reasoning: true,
            reasoning_options: vec!["none".into(), "high".into(), "xhigh".into()],
            context_window: Some(1_050_000),
            output_limit: Some(128_000),
            release_date: Some("2026-09-22".into()),
            last_updated: Some("2026-09-22".into()),
        }];
        let (preferences, models) =
            apply_models_dev(&mut snap, AssistantPreferences::default(), &catalog);

        assert_eq!(snap.active_model().unwrap().name, "gpt-6-luna");
        assert_eq!(preferences.active_model.as_deref(), Some("gpt-6-luna"));
        assert_eq!(preferences.active_effort.as_deref(), Some("none"));
        assert_eq!(models[0].display_name, "GPT-6 Luna");
        assert_eq!(models[0].effort_options, ["none", "high", "xhigh"]);
        assert_eq!(models[0].context_window, Some(1_050_000));
    }

    #[test]
    fn models_dev_keeps_selected_provider_id_when_kinds_are_duplicated() {
        let mut snap = ira_store::stub_snapshot("codex", "old-model", "prompt");
        let connected_id = Uuid::from_u128(2);
        snap.providers.push(ProviderRow {
            id: connected_id,
            name: "codex conectado".into(),
            kind: "codex".into(),
            base_url: None,
            api_key: Some("oauth-json".into()),
        });
        let catalog = vec![models_dev::CatalogModel {
            provider: "openai".into(),
            id: "gpt-6-luna".into(),
            name: "GPT-6 Luna".into(),
            reasoning: true,
            reasoning_options: vec!["high".into()],
            context_window: None,
            output_limit: None,
            release_date: None,
            last_updated: None,
        }];
        let preferences = AssistantPreferences {
            active_provider: Some("codex".into()),
            active_provider_id: None,
            active_model: Some("gpt-6-luna".into()),
            ..AssistantPreferences::default()
        };

        let (preferences, _) = apply_models_dev(&mut snap, preferences, &catalog);

        assert_eq!(snap.active_provider().unwrap().id, connected_id);
        let connected_id = connected_id.to_string();
        assert_eq!(
            preferences.active_provider_id.as_deref(),
            Some(connected_id.as_str())
        );
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

    #[test]
    fn chat_stream_events_use_tagged_snake_case_json() {
        assert_eq!(
            serde_json::to_value(ChatStreamEvent::Delta {
                text: "hola".into()
            })
            .unwrap(),
            serde_json::json!({"type": "delta", "text": "hola"})
        );
        assert_eq!(
            serde_json::to_value(ChatStreamEvent::Reset).unwrap(),
            serde_json::json!({"type": "reset"})
        );
        assert_eq!(
            serde_json::to_value(ChatStreamEvent::Done).unwrap(),
            serde_json::json!({"type": "done"})
        );
        assert_eq!(
            serde_json::to_value(ChatStreamEvent::Error {
                error: "falló".into()
            })
            .unwrap(),
            serde_json::json!({"type": "error", "error": "falló"})
        );
    }

    #[tokio::test]
    async fn conversation_turn_locks_are_scoped_by_id() {
        let app = App::unavailable("sin postgres");
        let id = Uuid::from_u128(1);
        let first = app.conversation_lock(id).await;
        let same = app.conversation_lock(id).await;
        let other = app.conversation_lock(Uuid::from_u128(2)).await;
        assert!(Arc::ptr_eq(&first, &same));
        assert!(!Arc::ptr_eq(&first, &other));

        let held = first.lock().await;
        let waiter = tokio::spawn(async move {
            let _acquired = same.lock().await;
        });
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());
        drop(held);
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("same-conversation waiter remained blocked")
            .unwrap();
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
