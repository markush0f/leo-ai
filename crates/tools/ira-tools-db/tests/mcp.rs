use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use ira_tools_db::Client;
use serde_json::{Value, json};

#[derive(Clone, Default)]
struct Mock {
    calls: Arc<AtomicU64>,
}

async fn mcp(
    State(mock): State<Mock>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    mock.calls.fetch_add(1, Ordering::Relaxed);
    let method = body.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        "initialize" => {
            let mut resp_headers = HeaderMap::new();
            resp_headers.insert("mcp-session-id", HeaderValue::from_static("sess-1"));
            let id = body.get("id").cloned().unwrap_or(json!(1));
            (
                StatusCode::OK,
                resp_headers,
                Json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2025-03-26",
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "toolbox", "version": "test" }
                    }
                })),
            )
                .into_response()
        }
        "notifications/initialized" => StatusCode::ACCEPTED.into_response(),
        "tools/list" => {
            assert_eq!(
                headers.get("mcp-session-id").and_then(|v| v.to_str().ok()),
                Some("sess-1")
            );
            let id = body.get("id").cloned().unwrap_or(json!(2));
            Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
                        {
                            "name": "execute_sql",
                            "description": "Run SQL",
                            "inputSchema": {
                                "type": "object",
                                "properties": { "sql": { "type": "string" } },
                                "required": ["sql"]
                            }
                        },
                        {
                            "name": "list_tables",
                            "description": "List tables",
                            "inputSchema": { "type": "object", "properties": {} }
                        }
                    ]
                }
            }))
            .into_response()
        }
        "tools/call" => {
            let params = body.get("params").cloned().unwrap_or(json!({}));
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let sql = params
                .pointer("/arguments/sql")
                .and_then(Value::as_str)
                .unwrap_or("");
            let text = if name == "execute_sql" {
                json!({ "ok": true, "sql": sql, "rows": [{"n": 1}] }).to_string()
            } else {
                json!({ "ok": true, "tool": name }).to_string()
            };
            let id = body.get("id").cloned().unwrap_or(json!(3));
            Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                }
            }))
            .into_response()
        }
        other => {
            let id = body.get("id").cloned().unwrap_or(json!(0));
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("unknown method {other}") }
                })),
            )
                .into_response()
        }
    }
}

async fn serve() -> (String, Mock) {
    let mock = Mock::default();
    let app = axum::Router::new()
        .route("/mcp", post(mcp))
        .with_state(mock.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), mock)
}

#[tokio::test]
async fn lists_and_invokes_over_mcp() {
    let (base, _) = serve().await;
    let client = Client::new(base, None, None);
    assert!(client.mcp_url().ends_with("/mcp"));

    let listed = ira_tools_db::list_tools::run(&client).await.unwrap();
    assert_eq!(listed["ok"], true);
    assert_eq!(listed["count"], 2);
    let names: Vec<&str> = listed["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"execute_sql"));

    let exec = ira_tools_db::execute_sql::run(&client, "select 1")
        .await
        .unwrap();
    assert_eq!(exec["ok"], true);
    assert_eq!(exec["result"]["sql"], "select 1");
    assert_eq!(exec["result"]["rows"][0]["n"], 1);

    let tables = ira_tools_db::list_tables::run(&client, None).await.unwrap();
    assert_eq!(tables["result"]["tool"], "list_tables");
}

#[tokio::test]
async fn rpc_error_is_surfaced() {
    let (base, _) = serve().await;
    let client = Client::new(format!("{base}/mcp/missing"), None, None);
    let err = client.list_tools().await.unwrap_err();
    assert!(err.to_string().contains("http") || err.to_string().contains("rpc"));
}

#[test]
fn specs_use_db_prefix() {
    assert_eq!(ira_tools_db::list_tools::spec().name, "db_list_tools");
    assert_eq!(ira_tools_db::invoke::spec().name, "db_invoke");
    assert_eq!(ira_tools_db::execute_sql::spec().name, "db_execute_sql");
    assert_eq!(ira_tools_db::list_tables::spec().name, "db_list_tables");
    assert_eq!(ira_tools_db::list_schemas::spec().name, "db_list_schemas");
    assert_eq!(ira_tools_db::list_views::spec().name, "db_list_views");
    assert_eq!(
        ira_tools_db::get_query_plan::spec().name,
        "db_get_query_plan"
    );
    assert_eq!(
        ira_tools_db::database_overview::spec().name,
        "db_database_overview"
    );
}
