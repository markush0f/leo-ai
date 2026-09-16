//! Native bridge for the React desktop application (`leo_desktop_lib`).
//!
//! Tauri commands expose catalog DTOs and chat turns. Provider secrets stay
//! behind the DTO boundary: the frontend receives only availability status
//! (`db` / `env` / `falta` / `none`). Chat uses PostgreSQL. Voice is deferred:
//! the catalog still stores engine columns, but this surface does not control
//! `leo-daemon`. The OS entry is `src/main.rs`.
//!
//! # Workspace crates
//!
//! - [`leo_store`] — catalog (providers, models, engines, settings, secrets)
//!   and local conversations. Setup runs `connect`, `migrate`,
//!   `apply_secrets_to_env`, and `sync_ollama_providers`. Commands map
//!   frontend `Op` values to [`leo_store::DbOp`], load
//!   [`leo_store::Snapshot`], and persist turns with `append_message` /
//!   `context_messages`.
//! - [`leo_llm`] — `load_dotenv` and [`leo_llm::Client`] from
//!   `Snapshot::client`. [`leo_llm::ChatRequest`] carries history; Grok turns
//!   set `reasoning_effort` from the thinking flag. Tool calls are data.
//! - [`leo_tools`] — [`leo_tools::Registry::from_env`] at startup.
//!   [`leo_tools::chat`] runs the tool loop when tools are enabled.

use leo_llm::{ChatRequest, ProviderId};
use leo_store::{
    self as db, CHANNEL_LOCAL, CONTEXT_LIMIT, ConversationRow, DbOp, EngineRole, MessageRow,
    NewMessage, ProviderRow, Snapshot,
};
use leo_tools::Registry;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tauri::Manager;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    pool: Option<PgPool>,
    db_error: Option<String>,
    tools: Registry,
}

#[derive(Serialize)]
struct SnapshotDto {
    providers: Vec<ProviderDto>,
    models: Vec<ModelDto>,
    engines: Vec<EngineDto>,
    active_model_id: Option<Uuid>,
    active_conversation_id: Option<Uuid>,
    system: String,
    voice_system: String,
    stt_engine_id: Option<Uuid>,
    tts_engine_id: Option<Uuid>,
    wake_engine_id: Option<Uuid>,
    stt_language: String,
    thinking: bool,
    tools_enabled: bool,
    tools: Vec<String>,
}

#[derive(Serialize)]
struct EngineDto {
    id: Uuid,
    role: String,
    kind: String,
    name: String,
}

#[derive(Serialize)]
struct ConversationDto {
    id: Uuid,
    title: Option<String>,
}

#[derive(Serialize)]
struct TurnDto {
    id: Uuid,
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ProviderDto {
    id: Uuid,
    name: String,
    kind: String,
    base_url: Option<String>,
    key: &'static str,
}

#[derive(Serialize)]
struct ModelDto {
    id: Uuid,
    provider_id: Uuid,
    name: String,
}

#[derive(Serialize)]
struct ChatOut {
    text: String,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Op {
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
fn dto(snap: Snapshot, tools: &[String]) -> SnapshotDto {
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

fn conv_dto(row: ConversationRow) -> ConversationDto {
    ConversationDto {
        id: row.id,
        title: row.title,
    }
}

fn turn_dto(row: MessageRow) -> TurnDto {
    TurnDto {
        id: row.id,
        role: row.role,
        content: row.content,
    }
}

fn key_status(provider: &ProviderRow) -> &'static str {
    if provider
        .api_key
        .as_ref()
        .is_some_and(|k| !k.trim().is_empty())
    {
        return "db";
    }
    if let Ok(kind) = ProviderId::parse(&provider.kind) {
        if let Some(var) = kind.env_key() {
            if std::env::var(var).is_ok_and(|v| !v.trim().is_empty()) {
                return "env";
            }
            return "falta";
        }
    }
    "none"
}

fn pool(state: &AppState) -> Result<&PgPool, String> {
    state.pool.as_ref().ok_or_else(|| {
        state
            .db_error
            .clone()
            .unwrap_or_else(|| "sin postgres".into())
    })
}

#[tauri::command]
async fn snapshot(state: tauri::State<'_, AppState>) -> Result<SnapshotDto, String> {
    let pool = pool(&state)?;
    let _ = db::sync_ollama_providers(pool).await;
    let snap = db::load(pool).await.map_err(|e| e.to_string())?;
    Ok(dto(snap, &state.tools.names()))
}

#[tauri::command]
async fn apply(state: tauri::State<'_, AppState>, op: Op) -> Result<SnapshotDto, String> {
    let pool = pool(&state)?;
    let snap = db::apply(pool, op.into())
        .await
        .map_err(|e| e.to_string())?;
    Ok(dto(snap, &state.tools.names()))
}

#[tauri::command]
async fn list_chats(state: tauri::State<'_, AppState>) -> Result<Vec<ConversationDto>, String> {
    let pool = pool(&state)?;
    let rows = db::list_conversations(pool, CHANNEL_LOCAL)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(conv_dto).collect())
}

#[tauri::command]
async fn open_chat(state: tauri::State<'_, AppState>, id: Uuid) -> Result<Vec<TurnDto>, String> {
    let pool = pool(&state)?;
    db::set_active_conversation(pool, Some(id))
        .await
        .map_err(|e| e.to_string())?;
    let rows = db::conversation_messages(pool, id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(turn_dto).collect())
}

#[tauri::command]
async fn new_chat(state: tauri::State<'_, AppState>) -> Result<ConversationDto, String> {
    let pool = pool(&state)?;
    let row = db::new_local(pool).await.map_err(|e| e.to_string())?;
    Ok(conv_dto(row))
}

#[tauri::command]
async fn chat(
    state: tauri::State<'_, AppState>,
    conversation_id: Uuid,
    text: String,
) -> Result<ChatOut, String> {
    let pool = pool(&state)?;
    if db::uses_ollama(&db::load(pool).await.map_err(|e| e.to_string())?) {
        let _ = db::sync_ollama_providers(pool).await;
    }
    let snap = db::load(pool).await.map_err(|e| e.to_string())?;
    let client = snap.client().map_err(|e| match e {
        leo_llm::LlmError::MissingKey(var) => {
            format!("falta api key ({var}): ábrelo en catálogo")
        }
        other => other.to_string(),
    })?;
    db::append_message(pool, conversation_id, NewMessage::user(text))
        .await
        .map_err(|e| e.to_string())?;
    let history = db::context_messages(pool, conversation_id, CONTEXT_LIMIT)
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
        state.tools.clone()
    } else {
        leo_tools::Registry::default()
    };
    match leo_tools::chat(&client, req, &registry).await {
        Ok(resp) => {
            db::append_message(
                pool,
                conversation_id,
                NewMessage::assistant(resp.text.clone(), snap.active_model_id),
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok(ChatOut { text: resp.text })
        }
        Err(err) => {
            let _ =
                db::append_message(pool, conversation_id, NewMessage::error(err.to_string())).await;
            Err(err.to_string())
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            leo_llm::load_dotenv();
            let tools = Registry::from_env();
            let url = db::database_url();
            let (pool, db_error) = tauri::async_runtime::block_on(async {
                match db::connect(&url).await {
                    Ok(pool) => {
                        if let Err(err) = db::migrate(&pool).await {
                            return (None, Some(format!("migrate: {err}")));
                        }
                        let _ = db::apply_secrets_to_env(&pool).await;
                        let _ = db::sync_ollama_providers(&pool).await;
                        (Some(pool), None)
                    }
                    Err(err) => (
                        None,
                        Some(format!(
                            "no se pudo conectar a postgres ({url}): {err}. arranca la bbdd: docker compose up -d"
                        )),
                    ),
                }
            });
            app.manage(AppState {
                pool,
                db_error,
                tools,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            apply,
            list_chats,
            open_chat,
            new_chat,
            chat,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
