//! PostgreSQL catalog shared by chat, voice, and tools.
//!
//! Persists providers, models, engines, settings, secrets, and conversations.
//! The base schema lives in `deploy/postgres/init.sql`; additive changes are
//! versioned under `deploy/postgres/migrations/`.

mod codex;
mod conversations;
mod databases;
mod migrate;
mod secrets;

use ira_llm::{Client, LlmError, ProviderId};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub use codex::{
    PostgresCodexTokenStore, client_with_pool, sync_codex_provider, sync_codex_providers,
};
pub use conversations::{
    CHANNEL_LOCAL, CHANNEL_TELEGRAM, CHANNEL_VOICE, CONTEXT_LIMIT, ConversationRow, MessageRow,
    NewMessage, append_message, archive_conversation, context_messages, conversation_messages,
    create_conversation, display_kind, ensure_local, ensure_telegram, ensure_voice,
    get_conversation, list_conversations, new_local, new_telegram, set_active_conversation,
};
pub use databases::{
    DatabaseCipher, DatabaseConnectionRow, DatabaseError, DatabaseWrite,
    create_database_connection, database_connection, database_password, delete_database_connection,
    list_database_connections, set_database_test_result, update_database_connection,
};
pub use migrate::migrate;
pub use secrets::{SecretRow, apply_secrets_to_env, get_secret, list_secrets, set_secret};

pub const DEFAULT_DATABASE_URL: &str = "postgres://ira:ira@127.0.0.1:5439/ira?sslmode=disable";
const LEGACY_DEFAULT_DATABASE_URL: &str = "postgres://leo:leo@127.0.0.1:5439/leo?sslmode=disable";

pub const ENGINE_STT_GROK: Uuid = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0501);
pub const ENGINE_STT_NONE: Uuid = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0502);
pub const ENGINE_TTS_TONE: Uuid = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0601);
pub const ENGINE_WAKE_NONE: Uuid = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0701);

const DEFAULT_SYSTEM: &str = "Eres Ira, un asistente. Responde en español, claro y directo.";
const DEFAULT_VOICE_SYSTEM: &str = "Eres Ira, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineRole {
    Stt,
    Tts,
    Wake,
}

impl EngineRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stt => "stt",
            Self::Tts => "tts",
            Self::Wake => "wake",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "stt" => Some(Self::Stt),
            "tts" => Some(Self::Tts),
            "wake" => Some(Self::Wake),
            _ => None,
        }
    }

    fn column(self) -> &'static str {
        match self {
            Self::Stt => "stt_engine_id",
            Self::Tts => "tts_engine_id",
            Self::Wake => "wake_engine_id",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderRow {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelRow {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct EngineRow {
    pub id: Uuid,
    pub role: String,
    pub kind: String,
    pub name: String,
    pub provider_id: Option<Uuid>,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct SettingsRow {
    pub active_model_id: Option<Uuid>,
    pub system_prompt: String,
    pub voice_system_prompt: String,
    pub stt_engine_id: Option<Uuid>,
    pub tts_engine_id: Option<Uuid>,
    pub wake_engine_id: Option<Uuid>,
    pub audio_source: String,
    pub audio_sink: String,
    pub vad_hangover_ms: i32,
    pub barge_in: bool,
    pub barge_in_rms: f32,
    pub stt_language: String,
    pub thinking: bool,
    pub tools_enabled: bool,
    pub active_conversation_id: Option<Uuid>,
    pub telegram_token_set: bool,
    pub telegram_allow_users: Vec<i64>,
}

impl SettingsRow {
    fn stub(active_model_id: Option<Uuid>, system: String) -> Self {
        Self {
            active_model_id,
            system_prompt: system,
            voice_system_prompt: DEFAULT_VOICE_SYSTEM.into(),
            stt_engine_id: None,
            tts_engine_id: None,
            wake_engine_id: None,
            audio_source: "@DEFAULT_SOURCE@".into(),
            audio_sink: "@DEFAULT_SINK@".into(),
            vad_hangover_ms: 500,
            barge_in: true,
            barge_in_rms: 0.035,
            stt_language: "es".into(),
            thinking: false,
            tools_enabled: true,
            active_conversation_id: None,
            telegram_token_set: false,
            telegram_allow_users: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
/// In-memory catalog and settings snapshot; conversations are loaded separately.
///
/// Contains provider credentials. Interfaces should convert it to presentation
/// DTOs rather than exposing those keys directly.
pub struct Snapshot {
    pub providers: Vec<ProviderRow>,
    pub models: Vec<ModelRow>,
    pub engines: Vec<EngineRow>,
    pub settings: SettingsRow,
    pub active_model_id: Option<Uuid>,
    pub system: String,
}

#[derive(Debug, Clone)]
/// Persistent catalog mutation shared by chat interfaces.
pub enum DbOp {
    ActivateProvider(Uuid),
    ActivateModel(Uuid),
    SetKind {
        id: Uuid,
        kind: String,
    },
    SetSystem(String),
    SetApiKey {
        id: Uuid,
        api_key: String,
    },
    SetBaseUrl {
        id: Uuid,
        base_url: String,
    },
    NewProvider {
        name: String,
    },
    NewModel {
        provider_id: Uuid,
        name: String,
    },
    RenameProvider {
        id: Uuid,
        name: String,
    },
    RenameModel {
        id: Uuid,
        name: String,
    },
    DeleteProvider(Uuid),
    DeleteModel(Uuid),
    SetVoiceSystem(String),
    SetEngine {
        role: EngineRole,
        id: Uuid,
    },
    SetVoiceAudio {
        source: String,
        sink: String,
    },
    SetVad {
        hangover_ms: u32,
        barge_in: bool,
        barge_in_rms: f32,
    },
    SetSttLanguage(String),
    SetThinking(bool),
    SetToolsEnabled(bool),
    SetTelegram {
        token: Option<String>,
        allow_users: Vec<i64>,
    },
    SetSecret {
        key: String,
        value: String,
    },
}

impl Snapshot {
    pub fn active_model(&self) -> Option<&ModelRow> {
        let id = self.active_model_id?;
        self.models.iter().find(|m| m.id == id)
    }

    pub fn active_provider(&self) -> Option<&ProviderRow> {
        let model = self.active_model()?;
        self.providers.iter().find(|p| p.id == model.provider_id)
    }

    pub fn models_of(&self, provider_id: Uuid) -> Vec<&ModelRow> {
        self.models
            .iter()
            .filter(|m| m.provider_id == provider_id)
            .collect()
    }

    pub fn engines_of(&self, role: EngineRole) -> Vec<&EngineRow> {
        let role = role.as_str();
        self.engines.iter().filter(|e| e.role == role).collect()
    }

    pub fn engine(&self, role: EngineRole) -> Option<&EngineRow> {
        let id = match role {
            EngineRole::Stt => self.settings.stt_engine_id?,
            EngineRole::Tts => self.settings.tts_engine_id?,
            EngineRole::Wake => self.settings.wake_engine_id?,
        };
        self.engines.iter().find(|e| e.id == id)
    }

    pub fn activate_provider(&mut self, id: Uuid) {
        if let Some(model) = self.models.iter().find(|m| m.provider_id == id) {
            self.active_model_id = Some(model.id);
        } else {
            self.active_model_id = None;
        }
        self.settings.active_model_id = self.active_model_id;
    }

    /// Builds the active model's client with stored or environment credentials.
    ///
    /// Fails if the model, provider, or required key is missing; performs no network I/O.
    pub fn client(&self) -> Result<Client, LlmError> {
        let model = self
            .active_model()
            .ok_or_else(|| LlmError::Empty("modelo"))?;
        let provider = self
            .providers
            .iter()
            .find(|p| p.id == model.provider_id)
            .ok_or_else(|| LlmError::Empty("proveedor"))?;
        let kind = ProviderId::parse(&provider.kind)?;
        Client::connect(kind, provider.api_key.clone(), provider.base_url.clone())
            .map(|c| c.with_model(model.name.clone()))
    }

    pub fn grok_api_key(&self) -> Option<String> {
        self.providers
            .iter()
            .find(|p| p.kind.eq_ignore_ascii_case("grok"))
            .and_then(|p| nonempty_owned(p.api_key.clone()))
            .or_else(|| nonempty_owned(std::env::var("XAI_API_KEY").ok()))
    }
}

/// Resolves the Ira URL, its legacy predecessor, `DATABASE_URL`, then the local default.
pub fn database_url() -> String {
    if let Ok(url) = std::env::var("IRA_DATABASE_URL") {
        return url;
    }
    if let Ok(url) = std::env::var("LEO_DATABASE_URL") {
        return if url == LEGACY_DEFAULT_DATABASE_URL {
            DEFAULT_DATABASE_URL.to_string()
        } else {
            url
        };
    }
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string())
}

pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new().max_connections(5).connect(url).await
}

pub async fn load(pool: &PgPool) -> Result<Snapshot, sqlx::Error> {
    let providers =
        sqlx::query("SELECT id, name, kind, base_url, api_key FROM providers ORDER BY name")
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| ProviderRow {
                id: row.get("id"),
                name: row.get("name"),
                kind: row.get("kind"),
                base_url: row.get("base_url"),
                api_key: row.get("api_key"),
            })
            .collect();

    let models = sqlx::query("SELECT id, provider_id, name FROM models ORDER BY name")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| ModelRow {
            id: row.get("id"),
            provider_id: row.get("provider_id"),
            name: row.get("name"),
        })
        .collect();

    let engines = sqlx::query(
        "SELECT id, role, kind, name, provider_id, config FROM engines ORDER BY role, name",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| EngineRow {
        id: row.get("id"),
        role: row.get("role"),
        kind: row.get("kind"),
        name: row.get("name"),
        provider_id: row.get("provider_id"),
        config: row.get("config"),
    })
    .collect();

    let settings = load_settings(pool).await?;
    let active_model_id = settings.active_model_id;
    let system = settings.system_prompt.clone();

    Ok(Snapshot {
        providers,
        models,
        engines,
        settings,
        active_model_id,
        system,
    })
}

async fn load_settings(pool: &PgPool) -> Result<SettingsRow, sqlx::Error> {
    let Some(row) = sqlx::query(
        "SELECT active_model_id, system_prompt, voice_system_prompt,
                stt_engine_id, tts_engine_id, wake_engine_id,
                audio_source, audio_sink, vad_hangover_ms, barge_in, barge_in_rms,
                stt_language, thinking, tools_enabled, active_conversation_id,
                telegram_token, telegram_allow_users
         FROM settings WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?
    else {
        return Ok(SettingsRow::stub(None, DEFAULT_SYSTEM.into()));
    };

    let token: Option<String> = row.get("telegram_token");
    Ok(SettingsRow {
        active_model_id: row.get("active_model_id"),
        system_prompt: row.get("system_prompt"),
        voice_system_prompt: row.get("voice_system_prompt"),
        stt_engine_id: row.get("stt_engine_id"),
        tts_engine_id: row.get("tts_engine_id"),
        wake_engine_id: row.get("wake_engine_id"),
        audio_source: row.get("audio_source"),
        audio_sink: row.get("audio_sink"),
        vad_hangover_ms: row.get("vad_hangover_ms"),
        barge_in: row.get("barge_in"),
        barge_in_rms: row.get("barge_in_rms"),
        stt_language: row.get("stt_language"),
        thinking: row.get("thinking"),
        tools_enabled: row.get("tools_enabled"),
        active_conversation_id: row.get("active_conversation_id"),
        telegram_token_set: token.as_ref().is_some_and(|t| !t.trim().is_empty()),
        telegram_allow_users: row
            .try_get::<Vec<i64>, _>("telegram_allow_users")
            .unwrap_or_default(),
    })
}

pub async fn telegram_token(pool: &PgPool) -> Result<Option<String>, sqlx::Error> {
    let value: Option<String> =
        sqlx::query_scalar("SELECT telegram_token FROM settings WHERE id = 1")
            .fetch_optional(pool)
            .await?
            .flatten();
    Ok(nonempty_owned(value))
}

/// Applies a mutation and reloads the catalog.
///
/// Provider activation, URL changes, and kind changes attempt an Ollama sync;
/// sync failure does not prevent returning the persisted catalog.
pub async fn apply(pool: &PgPool, op: DbOp) -> Result<Snapshot, sqlx::Error> {
    let should_sync = matches!(
        op,
        DbOp::ActivateProvider(_) | DbOp::SetBaseUrl { .. } | DbOp::SetKind { .. }
    );
    match op {
        DbOp::ActivateProvider(id) => {
            let model_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM models WHERE provider_id = $1 ORDER BY name LIMIT 1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await?;
            set_active(pool, model_id).await?;
        }
        DbOp::ActivateModel(id) => set_active(pool, Some(id)).await?,
        DbOp::SetKind { id, kind } => {
            sqlx::query("UPDATE providers SET kind = $2 WHERE id = $1")
                .bind(id)
                .bind(kind)
                .execute(pool)
                .await?;
        }
        DbOp::SetSystem(text) => {
            sqlx::query("UPDATE settings SET system_prompt = $1 WHERE id = 1")
                .bind(text)
                .execute(pool)
                .await?;
        }
        DbOp::SetApiKey { id, api_key } => {
            let value = empty_to_none(&api_key);
            sqlx::query("UPDATE providers SET api_key = $2 WHERE id = $1")
                .bind(id)
                .bind(value)
                .execute(pool)
                .await?;
        }
        DbOp::SetBaseUrl { id, base_url } => {
            let value = empty_to_none(&base_url);
            sqlx::query("UPDATE providers SET base_url = $2 WHERE id = $1")
                .bind(id)
                .bind(value)
                .execute(pool)
                .await?;
        }
        DbOp::NewProvider { name } => {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO providers (id, name, kind) VALUES ($1, $2, 'grok')")
                .bind(id)
                .bind(name)
                .execute(pool)
                .await?;
        }
        DbOp::NewModel { provider_id, name } => {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO models (id, provider_id, name) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(provider_id)
                .bind(name)
                .execute(pool)
                .await?;
        }
        DbOp::RenameProvider { id, name } => {
            sqlx::query("UPDATE providers SET name = $2 WHERE id = $1")
                .bind(id)
                .bind(name)
                .execute(pool)
                .await?;
        }
        DbOp::RenameModel { id, name } => {
            sqlx::query("UPDATE models SET name = $2 WHERE id = $1")
                .bind(id)
                .bind(name)
                .execute(pool)
                .await?;
        }
        DbOp::DeleteProvider(id) => {
            sqlx::query("DELETE FROM providers WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?;
        }
        DbOp::DeleteModel(id) => {
            sqlx::query("DELETE FROM models WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?;
        }
        DbOp::SetVoiceSystem(text) => {
            sqlx::query("UPDATE settings SET voice_system_prompt = $1 WHERE id = 1")
                .bind(text)
                .execute(pool)
                .await?;
        }
        DbOp::SetEngine { role, id } => {
            let sql = format!("UPDATE settings SET {} = $1 WHERE id = 1", role.column());
            sqlx::query(&sql).bind(id).execute(pool).await?;
        }
        DbOp::SetVoiceAudio { source, sink } => {
            sqlx::query("UPDATE settings SET audio_source = $1, audio_sink = $2 WHERE id = 1")
                .bind(source)
                .bind(sink)
                .execute(pool)
                .await?;
        }
        DbOp::SetVad {
            hangover_ms,
            barge_in,
            barge_in_rms,
        } => {
            sqlx::query(
                "UPDATE settings SET vad_hangover_ms = $1, barge_in = $2, barge_in_rms = $3 WHERE id = 1",
            )
            .bind(hangover_ms as i32)
            .bind(barge_in)
            .bind(barge_in_rms)
            .execute(pool)
            .await?;
        }
        DbOp::SetSttLanguage(text) => {
            sqlx::query("UPDATE settings SET stt_language = $1 WHERE id = 1")
                .bind(text)
                .execute(pool)
                .await?;
        }
        DbOp::SetThinking(value) => {
            sqlx::query("UPDATE settings SET thinking = $1 WHERE id = 1")
                .bind(value)
                .execute(pool)
                .await?;
        }
        DbOp::SetToolsEnabled(value) => {
            sqlx::query("UPDATE settings SET tools_enabled = $1 WHERE id = 1")
                .bind(value)
                .execute(pool)
                .await?;
        }
        DbOp::SetTelegram { token, allow_users } => {
            sqlx::query(
                "UPDATE settings SET telegram_token = $1, telegram_allow_users = $2 WHERE id = 1",
            )
            .bind(token.as_deref().and_then(empty_to_none))
            .bind(&allow_users)
            .execute(pool)
            .await?;
        }
        DbOp::SetSecret { key, value } => {
            set_secret(pool, &key, &value).await?;
        }
    }
    if should_sync {
        let _ = sync_ollama_providers(pool).await;
    }
    load(pool).await
}

pub fn uses_ollama(snap: &Snapshot) -> bool {
    snap.active_provider()
        .is_some_and(|p| p.kind.eq_ignore_ascii_case("ollama"))
}

pub async fn sync_ollama_providers(pool: &PgPool) -> Result<(), String> {
    let snap = load(pool).await.map_err(|e| e.to_string())?;
    for provider in &snap.providers {
        if !provider.kind.eq_ignore_ascii_case("ollama") {
            continue;
        }
        let origin = ollama_origin(provider.base_url.as_deref());
        if provider.base_url.as_deref() != Some(origin.as_str()) {
            sqlx::query("UPDATE providers SET base_url = $2 WHERE id = $1")
                .bind(provider.id)
                .bind(&origin)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        let Ok(kind) = ProviderId::parse(&provider.kind) else {
            continue;
        };
        let Ok(client) = Client::connect(kind, provider.api_key.clone(), Some(origin)) else {
            continue;
        };
        let Ok(names) = client.list_models().await else {
            continue;
        };
        replace_models(pool, provider.id, &names)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ollama_origin(base: Option<&str>) -> String {
    let raw = base.unwrap_or("http://127.0.0.1:11434");
    let t = raw.trim().trim_end_matches('/');
    t.strip_suffix("/v1")
        .unwrap_or(t)
        .trim_end_matches('/')
        .to_string()
}

async fn replace_models(
    pool: &PgPool,
    provider_id: Uuid,
    names: &[String],
) -> Result<(), sqlx::Error> {
    let existing = sqlx::query("SELECT id, name FROM models WHERE provider_id = $1")
        .bind(provider_id)
        .fetch_all(pool)
        .await?;
    let existing: Vec<(Uuid, String)> = existing
        .into_iter()
        .map(|row| (row.get("id"), row.get("name")))
        .collect();

    let active: Option<Uuid> =
        sqlx::query_scalar::<_, Option<Uuid>>("SELECT active_model_id FROM settings WHERE id = 1")
            .fetch_optional(pool)
            .await?
            .flatten();
    let active_was_ours = existing.iter().any(|(id, _)| Some(*id) == active);
    let active_name = existing
        .iter()
        .find(|(id, _)| Some(*id) == active)
        .map(|(_, name)| name.clone());

    for name in names {
        if existing.iter().any(|(_, n)| n == name) {
            continue;
        }
        sqlx::query("INSERT INTO models (id, provider_id, name) VALUES ($1, $2, $3)")
            .bind(Uuid::new_v4())
            .bind(provider_id)
            .bind(name)
            .execute(pool)
            .await?;
    }

    for (id, name) in &existing {
        if !names.iter().any(|n| n == name) {
            sqlx::query("DELETE FROM models WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await?;
        }
    }

    if active_was_ours {
        let keep = active_name.filter(|n| names.iter().any(|x| x == n));
        let chosen = keep.as_deref().or_else(|| pick_preferred(names));
        if let Some(name) = chosen {
            let id: Option<Uuid> =
                sqlx::query_scalar("SELECT id FROM models WHERE provider_id = $1 AND name = $2")
                    .bind(provider_id)
                    .bind(name)
                    .fetch_optional(pool)
                    .await?;
            if let Some(id) = id {
                set_active(pool, Some(id)).await?;
            }
        }
    }
    Ok(())
}

fn pick_preferred(names: &[String]) -> Option<&str> {
    names
        .iter()
        .find(|n| n.ends_with(":latest"))
        .or_else(|| names.first())
        .map(String::as_str)
}

async fn set_active(pool: &PgPool, model_id: Option<Uuid>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE settings SET active_model_id = $1 WHERE id = 1")
        .bind(model_id)
        .execute(pool)
        .await?;
    Ok(())
}

fn empty_to_none(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t) }
}

fn nonempty_owned(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    })
}

pub(crate) fn is_unique_violation(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .and_then(|e| e.code())
        .is_some_and(|c| c.as_ref() == "23505")
}

pub fn stub_snapshot(provider: &str, model: &str, system: &str) -> Snapshot {
    let pid = Uuid::from_u128(1);
    let mid = Uuid::from_u128(2);
    let settings = SettingsRow::stub(Some(mid), system.into());
    Snapshot {
        providers: vec![ProviderRow {
            id: pid,
            name: provider.into(),
            kind: provider.into(),
            base_url: None,
            api_key: None,
        }],
        models: vec![ModelRow {
            id: mid,
            provider_id: pid,
            name: model.into(),
        }],
        engines: Vec::new(),
        settings,
        active_model_id: Some(mid),
        system: system.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activate_provider_picks_first_model() {
        let mut snap = stub_snapshot("grok", "grok-4.6", "x");
        let other = Uuid::from_u128(9);
        snap.providers.push(ProviderRow {
            id: other,
            name: "gpt".into(),
            kind: "gpt".into(),
            base_url: None,
            api_key: None,
        });
        snap.models.push(ModelRow {
            id: Uuid::from_u128(10),
            provider_id: other,
            name: "gpt-4.1".into(),
        });
        snap.activate_provider(other);
        assert_eq!(snap.active_model().unwrap().name, "gpt-4.1");
    }

    #[tokio::test]
    async fn postgres_migrate_roundtrip() {
        let url = database_url();
        let Ok(pool) = connect(&url).await else {
            return;
        };
        migrate(&pool).await.expect("migrate");
        migrate(&pool).await.expect("migrate twice");
        let snap = load(&pool).await.expect("load");
        assert!(snap.providers.iter().any(|p| p.kind == "grok"), "seed grok");
        assert!(snap.active_model().is_some());
        assert!(
            snap.engines
                .iter()
                .any(|e| e.role == "stt" && e.kind == "grok"),
            "seed stt"
        );
        assert!(snap.settings.stt_engine_id.is_some());

        let name = format!("test-{}", Uuid::new_v4());
        let snap = apply(&pool, DbOp::NewProvider { name: name.clone() })
            .await
            .expect("new provider");
        let id = snap
            .providers
            .iter()
            .find(|p| p.name == name)
            .expect("inserted")
            .id;
        apply(&pool, DbOp::DeleteProvider(id))
            .await
            .expect("cleanup");
    }

    #[tokio::test]
    async fn conversation_roundtrip() {
        let url = database_url();
        let Ok(pool) = connect(&url).await else {
            return;
        };
        migrate(&pool).await.expect("migrate");
        let conv = new_local(&pool).await.expect("local");
        append_message(&pool, conv.id, NewMessage::user("hola mundo"))
            .await
            .expect("user");
        append_message(
            &pool,
            conv.id,
            NewMessage::assistant("hola", snap_model_id(&pool).await),
        )
        .await
        .expect("assistant");
        let ctx = context_messages(&pool, conv.id, 80).await.expect("ctx");
        assert_eq!(ctx.len(), 2);
        assert_eq!(ctx[0].content, "hola mundo");
        let listed = list_conversations(&pool, CHANNEL_LOCAL)
            .await
            .expect("list");
        assert!(listed.iter().any(|c| c.id == conv.id));
        assert_eq!(
            listed
                .iter()
                .find(|c| c.id == conv.id)
                .unwrap()
                .title
                .as_deref(),
            Some("hola mundo")
        );
    }

    #[tokio::test]
    async fn telegram_clear_opens_new_live_thread() {
        let url = database_url();
        let Ok(pool) = connect(&url).await else {
            return;
        };
        migrate(&pool).await.expect("migrate");
        let chat_id = -i64::from(Uuid::new_v4().as_u128() as u32);
        let first = ensure_telegram(&pool, chat_id).await.expect("first");
        let second = new_telegram(&pool, chat_id).await.expect("second");
        assert_ne!(first.id, second.id);
        let live = ensure_telegram(&pool, chat_id).await.expect("live");
        assert_eq!(live.id, second.id);
    }

    #[tokio::test]
    async fn set_engine_roundtrip() {
        let url = database_url();
        let Ok(pool) = connect(&url).await else {
            return;
        };
        migrate(&pool).await.expect("migrate");
        let snap = apply(
            &pool,
            DbOp::SetEngine {
                role: EngineRole::Stt,
                id: ENGINE_STT_NONE,
            },
        )
        .await
        .expect("set");
        assert_eq!(snap.settings.stt_engine_id, Some(ENGINE_STT_NONE));
        apply(
            &pool,
            DbOp::SetEngine {
                role: EngineRole::Stt,
                id: ENGINE_STT_GROK,
            },
        )
        .await
        .expect("restore");
    }

    async fn snap_model_id(pool: &PgPool) -> Option<Uuid> {
        load(pool).await.ok().and_then(|s| s.active_model_id)
    }

    #[test]
    fn prefers_latest_tag() {
        let names = vec!["gemma3:4b".into(), "gemma3:latest".into()];
        assert_eq!(pick_preferred(&names), Some("gemma3:latest"));
    }

    #[tokio::test]
    async fn sync_replaces_missing_ollama_model() {
        let url = database_url();
        let Ok(pool) = connect(&url).await else {
            return;
        };
        migrate(&pool).await.expect("migrate");
        let client = Client::from_env(ProviderId::Ollama).unwrap();
        let Ok(names) = client.list_models().await else {
            return;
        };
        if names.is_empty() {
            return;
        }
        sync_ollama_providers(&pool).await.expect("sync");
        let snap = load(&pool).await.expect("load");
        let ollama = snap
            .providers
            .iter()
            .find(|p| p.kind == "ollama")
            .expect("ollama provider");
        let models: Vec<String> = snap
            .models_of(ollama.id)
            .into_iter()
            .map(|m| m.name.clone())
            .collect();
        assert!(
            names.iter().all(|n| models.contains(n)),
            "sync names={names:?} db={models:?}"
        );
        assert!(!models.iter().any(|n| n == "llama3.2"));
    }
}
