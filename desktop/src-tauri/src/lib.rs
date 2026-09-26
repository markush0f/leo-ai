//! Native bridge for the React desktop application (`ira_desktop_lib`).
//!
//! Tauri commands forward to [`ira_api::App`], the same service `ira-server`
//! exposes over HTTP. Provider secrets stay behind the DTO boundary: the
//! frontend receives only availability status (`db` / `env` / `falta` / `none`).
//! `services` / `start_services` / `set_service` run Docker Compose.
//! Voice is deferred: this surface does not control `ira-daemon`.
//!
//! # Workspace crates
//!
//! - [`ira_api`] — catalog DTOs, local conversations, and chat with the tool
//!   loop. Setup is [`ira_api::App::boot`].

use ira_api::{
    App, ChatOut, ChatStreamEvent, ChatStreamSink, CodexLoginDto, ConversationDto, Op, ServicesDto,
    SnapshotDto, TurnDto, WhatsAppDto,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri::ipc::Channel;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    api: App,
}

#[derive(Default)]
struct StreamDelivery {
    error: Mutex<Option<String>>,
    terminal_delivered: AtomicBool,
}

impl StreamDelivery {
    fn record(&self, terminal: bool, result: Result<(), String>) {
        match result {
            Ok(()) if terminal => self.terminal_delivered.store(true, Ordering::Release),
            Ok(()) => {}
            Err(error) => {
                let mut stored = self.error.lock().unwrap();
                if stored.is_none() {
                    *stored = Some(error);
                }
            }
        }
    }

    fn finish(&self) -> Result<(), String> {
        if let Some(error) = self.error.lock().unwrap().take() {
            return Err(format!("no se pudo entregar el stream: {error}"));
        }
        if !self.terminal_delivered.load(Ordering::Acquire) {
            return Err("el stream terminó sin entregar un evento terminal".into());
        }
        Ok(())
    }
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
async fn set_service(
    state: tauri::State<'_, AppState>,
    id: String,
    action: String,
) -> Result<ServicesDto, String> {
    Ok(state.api.set_service(&id, &action).await)
}

#[tauri::command]
async fn whatsapp_status(state: tauri::State<'_, AppState>) -> Result<WhatsAppDto, String> {
    Ok(state.api.whatsapp_status().await)
}

#[tauri::command]
async fn whatsapp_start(state: tauri::State<'_, AppState>) -> Result<WhatsAppDto, String> {
    Ok(state.api.whatsapp_start().await)
}

#[tauri::command]
async fn whatsapp_pair(state: tauri::State<'_, AppState>) -> Result<WhatsAppDto, String> {
    Ok(state.api.whatsapp_pair().await)
}

#[tauri::command]
async fn whatsapp_allow(
    state: tauri::State<'_, AppState>,
    phones: String,
) -> Result<WhatsAppDto, String> {
    Ok(state.api.whatsapp_allow(phones).await)
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
async fn chat_stream(
    state: tauri::State<'_, AppState>,
    conversation_id: Uuid,
    text: String,
    sink: Channel<ChatStreamEvent>,
) -> Result<(), String> {
    let delivery = Arc::new(StreamDelivery::default());
    let callback_delivery = delivery.clone();
    let stream_sink: ChatStreamSink = Arc::new(move |event| {
        let terminal = matches!(event, ChatStreamEvent::Done | ChatStreamEvent::Error { .. });
        callback_delivery.record(
            terminal,
            sink.send(event).map_err(|error| error.to_string()),
        );
    });
    state
        .api
        .chat_stream(conversation_id, text, stream_sink)
        .await;

    delivery.finish()
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
            chat_stream,
            services,
            start_services,
            set_service,
            whatsapp_status,
            whatsapp_start,
            whatsapp_pair,
            whatsapp_allow,
            list_databases,
            create_database,
            update_database,
            delete_database,
            test_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_delivery_requires_successful_terminal_event() {
        let delivery = StreamDelivery::default();
        delivery.record(false, Ok(()));
        assert!(delivery.finish().unwrap_err().contains("evento terminal"));

        delivery.record(true, Ok(()));
        assert!(delivery.finish().is_ok());
    }

    #[test]
    fn stream_delivery_preserves_channel_failure() {
        let delivery = StreamDelivery::default();
        delivery.record(false, Err("webview cerrada".into()));
        delivery.record(true, Ok(()));
        assert!(delivery.finish().unwrap_err().contains("webview cerrada"));
    }
}
