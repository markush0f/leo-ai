use axum::{Json, Router, routing::post};
use ira_mcp::{McpManager, McpServerConfig};
use serde_json::{Value, json};

#[tokio::test]
async fn connects_and_calls_at_the_configured_url() {
    let app = Router::new().route(
        "/api/custom/endpoint",
        post(|Json(body): Json<Value>| async move {
            let result = match body["method"].as_str().unwrap_or("") {
                "initialize" => json!({"protocolVersion": "2025-03-26", "capabilities": {}}),
                "tools/list" => {
                    json!({"tools": [{"name": "weather", "inputSchema": {"type": "object"}}]})
                }
                "tools/call" => json!({"content": [{"type": "text", "text": "Sunny"}]}),
                _ => return Json(json!({})),
            };
            Json(json!({"jsonrpc": "2.0", "id": body["id"], "result": result}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, app).into_future());
    let manager = McpManager::new();
    let config = McpServerConfig::http("test", format!("http://{address}/api/custom/endpoint"));
    manager.connect(&config).await.unwrap();
    assert_eq!(manager.list_tools("test").await.unwrap()[0].name, "weather");
    assert_eq!(
        manager
            .call_tool("test", "weather", json!({}))
            .await
            .unwrap(),
        "Sunny"
    );
    manager.disconnect("test").await;
    server.abort();
}
