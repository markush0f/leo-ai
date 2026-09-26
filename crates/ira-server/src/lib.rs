//! HTTP surface for [`ira_api::App`].
//!
//! Browsers cannot call Ollama (CORS). This process does: it owns the catalog
//! and provider HTTP, then returns the same DTOs as the Tauri bridge.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{ConnectInfo, Path, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use futures::stream;
use ira_api::{App, ChatStreamEvent, DatabaseExportRequest, DatabaseInput, Op};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Serialize)]
pub struct Health {
    pub ok: bool,
    pub db: bool,
}

#[derive(Deserialize)]
struct ChatIn {
    text: String,
}

#[derive(Deserialize)]
struct WhatsAppAllowIn {
    phones: String,
}

#[derive(Deserialize)]
struct CodexLoginIn {
    provider_id: Uuid,
}

struct ChatStreamState {
    receiver: tokio::sync::mpsc::Receiver<ChatStreamEvent>,
    relay: tokio::task::AbortHandle,
}

impl Drop for ChatStreamState {
    fn drop(&mut self) {
        self.relay.abort();
    }
}

pub fn router(app: App, web_root: Option<PathBuf>) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/services", get(services).post(start_services))
        .route("/snapshot", get(snapshot))
        .route("/apply", post(apply))
        .route("/databases", get(list_databases).post(create_database))
        .route(
            "/databases/{id}",
            put(update_database).delete(delete_database),
        )
        .route("/databases/{id}/test", post(test_database))
        .route("/databases/{id}/json", get(export_database))
        .route("/databases/{id}/schema", get(export_database_schema))
        .route("/codex/login", post(begin_codex_login))
        .route("/codex/login/{id}/finish", post(finish_codex_login))
        .route("/chats", get(list_chats).post(new_chat))
        .route("/chats/{id}", get(open_chat))
        .route("/chats/{id}/messages/stream", post(chat_stream))
        .route("/chats/{id}/messages", post(chat))
        .route("/channels/whatsapp/{jid}", post(ensure_whatsapp))
        .route("/channels/whatsapp/{jid}/reset", post(reset_whatsapp))
        .route("/whatsapp", get(whatsapp_status).post(whatsapp_start))
        .route("/whatsapp/pair", post(whatsapp_pair))
        .route("/whatsapp/allow", put(whatsapp_allow))
        .layer(middleware::from_fn(local_whatsapp_admin));

    let mut router = Router::new()
        .nest("/api", api)
        .with_state(app)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::very_permissive());

    if let Some(root) = web_root.filter(|p| p.join("index.html").is_file()) {
        let index = ServeFile::new(root.join("index.html"));
        router = router.fallback_service(ServeDir::new(root).fallback(index));
    }

    router
}

async fn health(State(app): State<App>) -> Json<Health> {
    Json(Health {
        ok: true,
        db: app.db_ok().await,
    })
}

async fn services(State(app): State<App>) -> Response {
    send(Ok(app.services().await))
}

async fn start_services(State(app): State<App>) -> Response {
    send(Ok(app.start_services().await))
}

async fn snapshot(State(app): State<App>) -> Response {
    send(app.snapshot().await)
}

async fn apply(State(app): State<App>, Json(op): Json<Op>) -> Response {
    send(app.apply(op).await)
}

async fn list_databases(State(app): State<App>) -> Response {
    send(app.list_databases().await)
}

async fn create_database(State(app): State<App>, Json(input): Json<DatabaseInput>) -> Response {
    send(app.create_database(input).await)
}

async fn update_database(
    State(app): State<App>,
    Path(id): Path<Uuid>,
    Json(input): Json<DatabaseInput>,
) -> Response {
    send(app.update_database(id, input).await)
}

async fn delete_database(State(app): State<App>, Path(id): Path<Uuid>) -> Response {
    send(app.delete_database(id).await)
}

async fn test_database(State(app): State<App>, Path(id): Path<Uuid>) -> Response {
    send(app.test_database(id).await)
}

async fn export_database(State(app): State<App>, Path(id): Path<Uuid>) -> Response {
    send(
        app.export_database(id, DatabaseExportRequest::default())
            .await,
    )
}

async fn export_database_schema(State(app): State<App>, Path(id): Path<Uuid>) -> Response {
    send(
        app.export_database(
            id,
            DatabaseExportRequest {
                schema_only: true,
                ..DatabaseExportRequest::default()
            },
        )
        .await,
    )
}

async fn begin_codex_login(State(app): State<App>, Json(body): Json<CodexLoginIn>) -> Response {
    send(app.begin_codex_login(body.provider_id).await)
}

async fn finish_codex_login(State(app): State<App>, Path(id): Path<Uuid>) -> Response {
    send(app.finish_codex_login(id).await)
}

async fn list_chats(State(app): State<App>) -> Response {
    send(app.list_chats().await)
}

async fn new_chat(State(app): State<App>) -> Response {
    send(app.new_chat().await)
}

async fn ensure_whatsapp(State(app): State<App>, Path(jid): Path<String>) -> Response {
    send(app.ensure_whatsapp(&jid).await)
}

async fn reset_whatsapp(State(app): State<App>, Path(jid): Path<String>) -> Response {
    send(app.reset_whatsapp(&jid).await)
}

async fn local_whatsapp_admin(req: Request, next: Next) -> Response {
    let path = req.uri().path();
    let admin = path == "/whatsapp" || path.starts_with("/whatsapp/");
    if admin
        && let Some(ConnectInfo(addr)) = req.extensions().get::<ConnectInfo<SocketAddr>>()
        && !addr.ip().is_loopback()
    {
        return fail(
            StatusCode::FORBIDDEN,
            "WhatsApp solo se gestiona en esta máquina",
        );
    }
    next.run(req).await
}

async fn whatsapp_status(State(app): State<App>) -> Response {
    send(Ok(app.whatsapp_status().await))
}

async fn whatsapp_start(State(app): State<App>) -> Response {
    send(Ok(app.whatsapp_start().await))
}

async fn whatsapp_pair(State(app): State<App>) -> Response {
    send(Ok(app.whatsapp_pair().await))
}

async fn whatsapp_allow(State(app): State<App>, Json(body): Json<WhatsAppAllowIn>) -> Response {
    send(Ok(app.whatsapp_allow(body.phones).await))
}

async fn open_chat(State(app): State<App>, Path(id): Path<Uuid>) -> Response {
    send(app.open_chat(id).await)
}

async fn chat(State(app): State<App>, Path(id): Path<Uuid>, Json(body): Json<ChatIn>) -> Response {
    let text = body.text.trim();
    if text.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "mensaje vacío");
    }
    send(app.chat(id, text.to_string()).await)
}

async fn chat_stream(
    State(app): State<App>,
    Path(id): Path<Uuid>,
    Json(body): Json<ChatIn>,
) -> Response {
    let text = body.text.trim();
    if text.is_empty() {
        return fail(StatusCode::BAD_REQUEST, "mensaje vacío");
    }

    let (ingress_tx, mut ingress_rx) = tokio::sync::mpsc::unbounded_channel::<ChatStreamEvent>();
    let (body_tx, body_rx) = tokio::sync::mpsc::channel::<ChatStreamEvent>(32);
    let relay = tokio::spawn(async move {
        while let Some(event) = ingress_rx.recv().await {
            if body_tx.send(event).await.is_err() {
                break;
            }
        }
    });
    let sink = Arc::new(move |event| {
        let _ = ingress_tx.send(event);
    });
    let text = text.to_string();
    tokio::spawn(async move {
        app.chat_stream(id, text, sink).await;
    });

    let state = ChatStreamState {
        receiver: body_rx,
        relay: relay.abort_handle(),
    };
    let body = Body::from_stream(stream::unfold(state, |mut state| async move {
        let event = state.receiver.recv().await?;
        let mut line = serde_json::to_vec(&event).expect("stream event serialization");
        line.push(b'\n');
        Some((Ok::<_, std::convert::Infallible>(line), state))
    }));
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (header::CACHE_CONTROL, "no-cache"),
            (header::HeaderName::from_static("x-accel-buffering"), "no"),
        ],
        body,
    )
        .into_response()
}

fn send<T: Serialize>(result: Result<T, String>) -> Response {
    match result {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(msg) => fail(status_for(&msg), msg),
    }
}

fn fail(status: StatusCode, error: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorBody {
            error: error.into(),
        }),
    )
        .into_response()
}

fn status_for(msg: &str) -> StatusCode {
    if msg.contains("postgres") || msg.contains("sin postgres") || msg.starts_with("migrate:") {
        StatusCode::SERVICE_UNAVAILABLE
    } else if msg.contains("inexistente") {
        StatusCode::NOT_FOUND
    } else if msg.contains("obligatori")
        || msg.contains("fuera de rango")
        || msg.contains("modo SSL inválido")
        || msg.contains("conexión inválida")
        || msg.contains("jid de whatsapp inválido")
    {
        StatusCode::BAD_REQUEST
    } else if msg.contains("duplicate key") || msg.contains("database_connections_name_key") {
        StatusCode::CONFLICT
    } else {
        StatusCode::BAD_GATEWAY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, header};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_router() -> Router {
        router(App::unavailable("sin postgres"), None)
    }

    async fn body_json(resp: axum::http::Response<Body>) -> serde_json::Value {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn health_does_not_need_postgres() {
        let resp = test_router()
            .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["ok"], true);
        assert_eq!(json["db"], false);
    }

    #[tokio::test]
    async fn snapshot_without_db_is_unavailable() {
        let resp = test_router()
            .oneshot(Request::get("/api/snapshot").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let json = body_json(resp).await;
        assert!(json["error"].as_str().unwrap().contains("postgres"));
    }

    #[tokio::test]
    async fn codex_login_uses_shared_app_surface() {
        let resp = test_router()
            .oneshot(
                Request::post("/api/codex/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"provider_id":"00000000-0000-4000-8000-000000000003"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let json = body_json(resp).await;
        assert!(json["error"].as_str().unwrap().contains("postgres"));
    }

    #[tokio::test]
    async fn cors_allows_vite_origin() {
        let resp = test_router()
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/health")
                    .header(header::ORIGIN, "http://localhost:5179")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let allow = resp
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok());
        assert!(
            allow == Some("*") || allow == Some("http://localhost:5179"),
            "{allow:?}"
        );
    }

    #[tokio::test]
    async fn services_do_not_need_postgres() {
        let resp = test_router()
            .oneshot(Request::get("/api/services").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert!(json["services"].as_array().unwrap().len() >= 2);
        assert_eq!(json["services"][0]["id"], "postgres");
        assert_eq!(json["services"][1]["id"], "toolbox");
    }

    #[tokio::test]
    async fn database_json_ignores_table_filters() {
        let id = Uuid::from_u128(1);
        let resp = test_router()
            .oneshot(
                Request::get(format!(
                    "/api/databases/{id}/json?table=messages&table=conversations"
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let json = body_json(resp).await;
        assert!(json["error"].as_str().unwrap().contains("postgres"));
    }

    #[tokio::test]
    async fn database_schema_uses_shared_app_surface() {
        let id = Uuid::from_u128(1);
        let resp = test_router()
            .oneshot(
                Request::get(format!("/api/databases/{id}/schema"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let json = body_json(resp).await;
        assert!(json["error"].as_str().unwrap().contains("postgres"));
    }

    #[tokio::test]
    async fn whatsapp_status_without_bridge_is_down() {
        let resp = test_router()
            .oneshot(Request::get("/api/whatsapp").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert!(json["phase"].is_string());
        assert!(json["ok"].is_boolean());
    }

    #[tokio::test]
    async fn whatsapp_ensure_without_db_is_unavailable() {
        let resp = test_router()
            .oneshot(
                Request::post("/api/channels/whatsapp/34600000000%40s.whatsapp.net")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn whatsapp_jid_with_slash_is_rejected() {
        let resp = test_router()
            .oneshot(
                Request::post("/api/channels/whatsapp/34600000000%40s.whatsapp.net%2Fextra")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn empty_chat_is_rejected() {
        let id = Uuid::from_u128(1);
        let resp = test_router()
            .oneshot(
                Request::post(format!("/api/chats/{id}/messages"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"text":"   "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn empty_streaming_chat_is_rejected_before_stream() {
        let id = Uuid::from_u128(1);
        let resp = test_router()
            .oneshot(
                Request::post(format!("/api/chats/{id}/messages/stream"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"text":"   "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_ne!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/x-ndjson"
        );
    }

    #[tokio::test]
    async fn streaming_chat_returns_ndjson_immediately() {
        let id = Uuid::from_u128(1);
        let resp = test_router()
            .oneshot(
                Request::post(format!("/api/chats/{id}/messages/stream"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"text":"hola"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/x-ndjson"
        );
        assert_eq!(resp.headers().get("x-accel-buffering").unwrap(), "no");
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let event: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(event["type"], "error");
    }

    #[tokio::test]
    async fn dropping_stream_state_aborts_relay() {
        let (_tx, receiver) = tokio::sync::mpsc::channel(1);
        let relay = tokio::spawn(std::future::pending::<()>());
        let state = ChatStreamState {
            receiver,
            relay: relay.abort_handle(),
        };

        drop(state);
        assert!(relay.await.unwrap_err().is_cancelled());
    }
}
