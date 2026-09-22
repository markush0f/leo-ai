use ira_llm::ProviderId;
use ira_store::{
    ConversationRow, DatabaseConnectionRow, DbOp, EngineRole, MessageRow, ProviderRow, Snapshot,
};
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotDto {
    pub providers: Vec<ProviderDto>,
    pub models: Vec<ModelDto>,
    pub engines: Vec<EngineDto>,
    pub active_model_id: Option<Uuid>,
    pub active_conversation_id: Option<Uuid>,
    pub system: String,
    pub voice_system: String,
    pub stt_engine_id: Option<Uuid>,
    pub tts_engine_id: Option<Uuid>,
    pub wake_engine_id: Option<Uuid>,
    pub stt_language: String,
    pub thinking: bool,
    pub tools_enabled: bool,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineDto {
    pub id: Uuid,
    pub role: String,
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversationDto {
    pub id: Uuid,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TurnDto {
    pub id: Uuid,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderDto {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub key: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelDto {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatOut {
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodexLoginDto {
    pub id: Uuid,
    pub verification_url: String,
    pub user_code: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseConnectionDto {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub database: String,
    pub username: String,
    pub ssl_mode: String,
    pub enabled: bool,
    pub password_set: bool,
    pub last_test_ok: Option<bool>,
    pub last_test_error: Option<String>,
    pub last_tested_at: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct DatabaseInput {
    pub name: String,
    pub host: String,
    pub port: i32,
    pub database: String,
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    pub ssl_mode: String,
    pub enabled: bool,
}

/// Filtros de `GET /api/databases/{id}/json`.
///
/// `schema` y `table` aceptan un valor o una lista separada por comas.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DatabaseExportRequest {
    #[serde(default, rename = "schema", deserialize_with = "one_or_many")]
    pub schemas: Vec<String>,
    #[serde(default, rename = "table", deserialize_with = "one_or_many")]
    pub tables: Vec<String>,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub views: bool,
    #[serde(default)]
    pub schema_only: bool,
}

fn one_or_many<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        Many(Vec<String>),
        One(String),
    }

    let values = match OneOrMany::deserialize(deserializer)? {
        OneOrMany::Many(values) => values,
        OneOrMany::One(value) => vec![value],
    };
    Ok(values
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(|part| part.trim().to_string())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
        })
        .collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseTestDto {
    pub ok: bool,
    pub read_only: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteDto {
    pub ok: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    ActivateProvider { id: Uuid },
    ActivateModel { id: Uuid },
    SetKind { id: Uuid, kind: String },
    SetSystem { text: String },
    SetApiKey { id: Uuid, api_key: String },
    SetBaseUrl { id: Uuid, base_url: String },
    NewProvider { name: String },
    NewModel { provider_id: Uuid, name: String },
    RenameProvider { id: Uuid, name: String },
    RenameModel { id: Uuid, name: String },
    DeleteProvider { id: Uuid },
    DeleteModel { id: Uuid },
    SetVoiceSystem { text: String },
    SetEngine { role: String, id: Uuid },
    SetSttLanguage { text: String },
    SetThinking { value: bool },
    SetToolsEnabled { value: bool },
}

impl From<Op> for DbOp {
    fn from(op: Op) -> Self {
        match op {
            Op::ActivateProvider { id } => DbOp::ActivateProvider(id),
            Op::ActivateModel { id } => DbOp::ActivateModel(id),
            Op::SetKind { id, kind } => DbOp::SetKind { id, kind },
            Op::SetSystem { text } => DbOp::SetSystem(text),
            Op::SetApiKey { id, api_key } => DbOp::SetApiKey { id, api_key },
            Op::SetBaseUrl { id, base_url } => DbOp::SetBaseUrl { id, base_url },
            Op::NewProvider { name } => DbOp::NewProvider { name },
            Op::NewModel { provider_id, name } => DbOp::NewModel { provider_id, name },
            Op::RenameProvider { id, name } => DbOp::RenameProvider { id, name },
            Op::RenameModel { id, name } => DbOp::RenameModel { id, name },
            Op::DeleteProvider { id } => DbOp::DeleteProvider(id),
            Op::DeleteModel { id } => DbOp::DeleteModel(id),
            Op::SetVoiceSystem { text } => DbOp::SetVoiceSystem(text),
            Op::SetEngine { role, id } => DbOp::SetEngine {
                role: EngineRole::parse(&role).unwrap_or(EngineRole::Stt),
                id,
            },
            Op::SetSttLanguage { text } => DbOp::SetSttLanguage(text),
            Op::SetThinking { value } => DbOp::SetThinking(value),
            Op::SetToolsEnabled { value } => DbOp::SetToolsEnabled(value),
        }
    }
}

/// Converts internal rows into the frontend contract without returning API keys.
pub fn snapshot_dto(snap: Snapshot, tools: &[String]) -> SnapshotDto {
    SnapshotDto {
        providers: snap
            .providers
            .iter()
            .map(|p| ProviderDto {
                id: p.id,
                name: p.name.clone(),
                kind: p.kind.clone(),
                base_url: p.base_url.clone(),
                key: key_status(p),
            })
            .collect(),
        models: snap
            .models
            .iter()
            .map(|m| ModelDto {
                id: m.id,
                provider_id: m.provider_id,
                name: m.name.clone(),
            })
            .collect(),
        engines: snap
            .engines
            .iter()
            .map(|e| EngineDto {
                id: e.id,
                role: e.role.clone(),
                kind: e.kind.clone(),
                name: e.name.clone(),
            })
            .collect(),
        active_model_id: snap.active_model_id,
        active_conversation_id: snap.settings.active_conversation_id,
        system: snap.system,
        voice_system: snap.settings.voice_system_prompt.clone(),
        stt_engine_id: snap.settings.stt_engine_id,
        tts_engine_id: snap.settings.tts_engine_id,
        wake_engine_id: snap.settings.wake_engine_id,
        stt_language: snap.settings.stt_language.clone(),
        thinking: snap.settings.thinking,
        tools_enabled: snap.settings.tools_enabled,
        tools: tools.to_vec(),
    }
}

pub fn conversation_dto(row: ConversationRow) -> ConversationDto {
    ConversationDto {
        id: row.id,
        title: row.title,
    }
}

pub fn turn_dto(row: MessageRow) -> TurnDto {
    TurnDto {
        id: row.id,
        role: row.role,
        content: row.content,
    }
}

pub fn database_dto(row: DatabaseConnectionRow) -> DatabaseConnectionDto {
    DatabaseConnectionDto {
        id: row.id,
        name: row.name,
        host: row.host,
        port: row.port,
        database: row.database,
        username: row.username,
        ssl_mode: row.ssl_mode,
        enabled: row.enabled,
        password_set: row.password_set,
        last_test_ok: row.last_test_ok,
        last_test_error: row.last_test_error,
        last_tested_at: row.last_tested_at,
    }
}

pub(crate) fn key_status(provider: &ProviderRow) -> &'static str {
    if provider
        .api_key
        .as_ref()
        .is_some_and(|k| !k.trim().is_empty())
    {
        return "db";
    }
    if let Ok(kind) = ProviderId::parse(&provider.kind) {
        if kind == ProviderId::Codex {
            return "falta";
        }
        if let Some(var) = kind.env_key() {
            if std::env::var(var).is_ok_and(|v| !v.trim().is_empty()) {
                return "env";
            }
            return "falta";
        }
    }
    "none"
}
