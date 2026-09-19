//! Native bridge for the React desktop application (`leo_desktop_lib`).
//!
//! Tauri commands forward to [`leo_api::App`], the same service `leo-server`
//! exposes over HTTP. Provider secrets stay behind the DTO boundary: the
//! frontend receives only availability status (`db` / `env` / `falta` / `none`).
//! `services` / `start_services` run Docker Compose for Postgres and Toolbox.
//! Voice is deferred: this surface does not control `leo-daemon`.
//!
//! # Workspace crates
//!
//! - [`leo_api`] — catalog DTOs, local conversations, and chat with the tool
//!   loop. Setup is [`leo_api::App::boot`].

use leo_api::{
    App, ChatOut, CodexLoginDto, ConversationDto, Op, ServicesDto, SnapshotDto, TurnDto,
};
use tauri::Manager;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    api: App,
}

#[tauri::command]
async fn snapshot(state: tauri::State<'_, AppState>) -> Result<SnapshotDto, String> {
    state.api.snapshot().await
}

#[tauri::command]
async fn apply(state: tauri::State<'_, AppState>, op: Op) -> Result<SnapshotDto, String> {
    state.api.apply(op).await
}

#[tauri::command]
async fn begin_codex_login(
    state: tauri::State<'_, AppState>,
    provider_id: Uuid,
) -> Result<CodexLoginDto, String> {
    state.api.begin_codex_login(provider_id).await
}

#[tauri::command]
async fn finish_codex_login(
    state: tauri::State<'_, AppState>,
    id: Uuid,
) -> Result<SnapshotDto, String> {
    state.api.finish_codex_login(id).await
}

#[tauri::command]
async fn list_chats(state: tauri::State<'_, AppState>) -> Result<Vec<ConversationDto>, String> {
    state.api.list_chats().await
}

#[tauri::command]
async fn open_chat(state: tauri::State<'_, AppState>, id: Uuid) -> Result<Vec<TurnDto>, String> {
    state.api.open_chat(id).await
}

#[tauri::command]
async fn new_chat(state: tauri::State<'_, AppState>) -> Result<ConversationDto, String> {
    state.api.new_chat().await
}

#[tauri::command]
async fn services(state: tauri::State<'_, AppState>) -> Result<ServicesDto, String> {
    Ok(state.api.services().await)
}

#[tauri::command]
async fn start_services(state: tauri::State<'_, AppState>) -> Result<ServicesDto, String> {
    Ok(state.api.start_services().await)
}

#[tauri::command]
async fn chat(
    state: tauri::State<'_, AppState>,
    conversation_id: Uuid,
    text: String,
) -> Result<ChatOut, String> {
    state.api.chat(conversation_id, text).await
}

#[tauri::command]
async fn list_databases(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let databases = state.api.list_databases().await?;
    serde_json::to_value(databases).map_err(|err| err.to_string())
}

#[tauri::command]
async fn create_database(
    state: tauri::State<'_, AppState>,
    input: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let input = serde_json::from_value(input).map_err(|err| err.to_string())?;
    let database = state.api.create_database(input).await?;
    serde_json::to_value(database).map_err(|err| err.to_string())
}

#[tauri::command]
async fn update_database(
    state: tauri::State<'_, AppState>,
    id: Uuid,
    input: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let input = serde_json::from_value(input).map_err(|err| err.to_string())?;
    let database = state.api.update_database(id, input).await?;
    serde_json::to_value(database).map_err(|err| err.to_string())
}

#[tauri::command]
async fn delete_database(
    state: tauri::State<'_, AppState>,
    id: Uuid,
) -> Result<serde_json::Value, String> {
    state.api.delete_database(id).await?;
    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command]
async fn test_database(
    state: tauri::State<'_, AppState>,
    id: Uuid,
) -> Result<serde_json::Value, String> {
    let tested = state.api.test_database(id).await?;
    serde_json::to_value(tested).map_err(|err| err.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let api = tauri::async_runtime::block_on(App::boot());
            app.manage(AppState { api });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            apply,
            begin_codex_login,
            finish_codex_login,
            list_chats,
            open_chat,
            new_chat,
            chat,
            services,
            start_services,
            list_databases,
            create_database,
            update_database,
            delete_database,
            test_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
