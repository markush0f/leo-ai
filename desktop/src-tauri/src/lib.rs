use leo_ipc::{Request, Response};
use leo_llm::{ChatMessage, ChatRequest, ProviderId};
use leo_store::{self as db, DbOp, ProviderRow, Snapshot};
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
    active_model_id: Option<Uuid>,
    system: String,
    tools: Vec<String>,
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
struct FrontMsg {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct VoiceDto {
    running: bool,
    ok: bool,
    state: String,
    message: Option<String>,
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
        }
    }
}

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
        active_model_id: snap.active_model_id,
        system: snap.system,
        tools: tools.to_vec(),
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
async fn chat(
    state: tauri::State<'_, AppState>,
    messages: Vec<FrontMsg>,
    thinking: bool,
    tools: bool,
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
    let history = messages.into_iter().filter_map(|m| match m.role.as_str() {
        "user" => Some(ChatMessage::user(m.content)),
        "assistant" => Some(ChatMessage::assistant(m.content)),
        _ => None,
    });
    let mut req = ChatRequest::with_history(&snap.system, history);
    if snap
        .active_provider()
        .is_some_and(|p| p.kind.eq_ignore_ascii_case("grok"))
    {
        req.reasoning_effort = Some(if thinking { "high" } else { "low" }.into());
    }
    let registry = if tools {
        state.tools.clone()
    } else {
        leo_tools::Registry::default()
    };
    let resp = leo_tools::chat(&client, req, &registry)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ChatOut { text: resp.text })
}

#[tauri::command]
async fn voice_status() -> VoiceDto {
    voice_req(Request::Status).await
}

#[tauri::command]
async fn voice_listen() -> VoiceDto {
    voice_req(Request::Listen).await
}

#[tauri::command]
async fn voice_stop() -> VoiceDto {
    voice_req(Request::Stop).await
}

#[tauri::command]
async fn voice_shutdown() -> VoiceDto {
    voice_req(Request::Shutdown).await
}

#[tauri::command]
async fn voice_speak(text: String) -> VoiceDto {
    voice_req(Request::Speak { text }).await
}

async fn voice_req(req: Request) -> VoiceDto {
    match leo_ipc::send(&leo_ipc::socket_path(), &req).await {
        Ok(Response {
            ok,
            state,
            message,
        }) => VoiceDto {
            running: true,
            ok,
            state,
            message,
        },
        Err(_) => VoiceDto {
            running: false,
            ok: false,
            state: "apagado".into(),
            message: Some("el daemon no está en marcha".into()),
        },
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
            chat,
            voice_status,
            voice_listen,
            voice_stop,
            voice_shutdown,
            voice_speak
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
