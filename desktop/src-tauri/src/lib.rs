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

use leo_api::{App, ChatOut, ConversationDto, Op, ServicesDto, SnapshotDto, TurnDto};
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
            list_chats,
            open_chat,
            new_chat,
            chat,
            services,
            start_services,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
