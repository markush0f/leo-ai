//! HTTP surface for [`ira_api::App`].
//!
//! Browsers cannot call Ollama (CORS). This process does: it owns the catalog
//! and provider HTTP, then returns the same DTOs as the Tauri bridge.

mod token;

pub use token::load_http_token;

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use futures::stream;
use ira_api::{App, ChatStreamEvent, DatabaseExportRequest, DatabaseInput, Op};
use serde::{Deserialize, Serialize};
use tower_http::cors::{AllowOrigin, CorsLayer};
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
struct WebSearchIn {
    enabled: bool,
    context_size: String,
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

pub fn router(app: App, web_root: Option<PathBuf>, token: impl Into<String>) -> Router {
    let token = token.into();
    let api = Router::new()
        .route("/health", get(health))
        .route("/services", get(services).post(start_services))
        .route("/services/{id}", post(set_service))
        .route("/snapshot", get(snapshot))
        .route("/apply", post(apply))
        .route(
            "/preferences/web-search",
            axum::routing::put(update_web_search),
        )
        .route("/databases", get(list_databases).post(create_database))
        .route(
            "/databases/{id}",
            put(update_database).delete(delete_database),
        )
        .route("/databases/{id}/test", post(test_database))
        .route("/mcp", get(list_mcp).post(save_mcp))
        .route("/mcp/{id}", axum::routing::delete(delete_mcp))
        .route("/mcp/{id}/test", post(test_mcp))
        .route("/databases/{id}/json", get(export_database))
        .route("/databases/{id}/schema", get(export_database_schema))
        .route("/codex/login", post(begin_codex_login))
        .route("/codex/login/{id}/finish", post(finish_codex_login))
        .route("/chats", get(list_chats).post(new_chat))
        .route("/chats/{id}", get(open_chat))
        .route("/chats/{id}/messages/stream", post(chat_stream))
        .route("/chats/{id}/messages", post(chat));

    let guard = token.clone();
    let mut router = Router::new()
        .nest("/api", api)
        .with_state(app)
        .layer(middleware::from_fn(move |req, next| {
            let token = guard.clone();
            async move { authorize(token, req, next).await }
        }))
        .layer(TraceLayer::new_for_http())
        .layer(cors());

    if let Some(root) = web_root.filter(|p| p.join("index.html").is_file()) {
        let index = ServeFile::new(root.join("index.html"));
        router = router.fallback_service(ServeDir::new(root).fallback(index));
    }

    router
}

fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("http://127.0.0.1:5179"),
            HeaderValue::from_static("http://localhost:5179"),
        ]))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
}

async fn authorize(token: String, req: axum::extract::Request, next: Next) -> Response {
    let path = req.uri().path();
    let api = path.starts_with("/api");
    let health = req.method() == Method::GET && path == "/api/health";
    if api && !health && !authorized(&req, &token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorBody {
                error: "no autorizado".into(),
            }),
        )
            .into_response();
    }
    let mut response = next.run(req).await;
    if !api
        && let Ok(value) = HeaderValue::from_str(&format!(
            "ira_token={token}; HttpOnly; SameSite=Strict; Path=/"
        ))
    {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

fn authorized(req: &axum::extract::Request, token: &str) -> bool {
    if let Some(value) = req.headers().get(header::AUTHORIZATION)
        && let Ok(value) = value.to_str()
        && let Some(got) = value.strip_prefix("Bearer ")
        && token_eq(got.trim(), token)
    {
        return true;
    }
    if let Some(value) = req.headers().get(header::COOKIE)
        && let Ok(value) = value.to_str()
    {
        return value.split(';').any(|part| {
            part.trim()
                .strip_prefix("ira_token=")
                .is_some_and(|got| token_eq(got.trim(), token))
        });
    }
    false
}

fn token_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
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

async fn set_service(
    State(app): State<App>,
    Path(id): Path<String>,
    Json(body): Json<ServiceAction>,
) -> Response {
    send(Ok(app
        .set_service(
            &id,
            &body.action,
            body.port,
            body.name.as_deref(),
            body.description.as_deref(),
        )
        .await))
}

#[derive(Deserialize)]
struct ServiceAction {
    action: String,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

async fn snapshot(State(app): State<App>) -> Response {
    send(app.snapshot().await)
}

async fn list_mcp(State(app): State<App>) -> Response {
    send(app.list_mcp().await)
}

async fn save_mcp(State(app): State<App>, Json(input): Json<ira_api::McpInput>) -> Response {
    send(app.save_mcp(input).await)
}

async fn delete_mcp(State(app): State<App>, Path(id): Path<String>) -> Response {
    send(app.delete_mcp(&id).await)
}

async fn test_mcp(State(app): State<App>, Path(id): Path<String>) -> Response {
    send(app.test_mcp(&id).await)
}

async fn apply(State(app): State<App>, Json(op): Json<Op>) -> Response {
    send(app.apply(op).await)
}

async fn update_web_search(State(app): State<App>, Json(input): Json<WebSearchIn>) -> Response {
    send(app.update_web_search(input.enabled, &input.context_size))
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

    const TOKEN: &str = "test-token";

    fn test_router() -> Router {
        router(App::unavailable("sin postgres"), None, TOKEN)
    }

    fn authed(mut req: Request<Body>) -> Request<Body> {
        req.headers_mut().insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer test-token"),
        );
        req
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
    async fn snapshot_without_token_is_unauthorized() {
        let resp = test_router()
            .oneshot(Request::get("/api/snapshot").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn cookie_authorizes_api() {
        let resp = test_router()
            .oneshot(
                Request::get("/api/services")
                    .header(header::COOKIE, "ira_token=test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn snapshot_without_db_is_unavailable() {
        let resp = test_router()
            .oneshot(authed(
                Request::get("/api/snapshot").body(Body::empty()).unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let json = body_json(resp).await;
        assert!(json["error"].as_str().unwrap().contains("postgres"));
    }

    #[tokio::test]
    async fn codex_login_uses_shared_app_surface() {
        let resp = test_router()
            .oneshot(authed(
                Request::post("/api/codex/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"provider_id":"00000000-0000-4000-8000-000000000003"}"#,
                    ))
                    .unwrap(),
            ))
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
        assert!(allow == Some("http://localhost:5179"), "{allow:?}");
    }

    #[tokio::test]
    async fn cors_rejects_foreign_origin() {
        let resp = test_router()
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/snapshot")
                    .header(header::ORIGIN, "https://evil.example")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let allow = resp
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok());
        assert_ne!(allow, Some("https://evil.example"));
        assert_ne!(allow, Some("*"));
    }

    #[tokio::test]
    async fn services_do_not_need_postgres() {
        let resp = test_router()
            .oneshot(authed(
                Request::get("/api/services").body(Body::empty()).unwrap(),
            ))
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
            .oneshot(authed(
                Request::get(format!(
                    "/api/databases/{id}/json?table=messages&table=conversations"
                ))
                .body(Body::empty())
                .unwrap(),
            ))
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
            .oneshot(authed(
                Request::get(format!("/api/databases/{id}/schema"))
                    .body(Body::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let json = body_json(resp).await;
        assert!(json["error"].as_str().unwrap().contains("postgres"));
    }

    #[tokio::test]
    async fn empty_chat_is_rejected() {
        let id = Uuid::from_u128(1);
        let resp = test_router()
            .oneshot(authed(
                Request::post(format!("/api/chats/{id}/messages"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"text":"   "}"#))
                    .unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn empty_streaming_chat_is_rejected_before_stream() {
        let id = Uuid::from_u128(1);
        let resp = test_router()
            .oneshot(authed(
                Request::post(format!("/api/chats/{id}/messages/stream"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"text":"   "}"#))
                    .unwrap(),
            ))
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
            .oneshot(authed(
                Request::post(format!("/api/chats/{id}/messages/stream"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"text":"hola"}"#))
                    .unwrap(),
            ))
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
