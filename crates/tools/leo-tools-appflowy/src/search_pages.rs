use leo_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "appflowy_search_pages".into(),
        description: "Busca páginas en el workspace de AppFlowy.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer" }
            },
            "required": ["query"]
        }),
    }
}

pub async fn run(
    client: &Client,
    query: &str,
    limit: Option<u64>,
) -> Result<serde_json::Value, Error> {
    let workspace = client.workspace_id().await?;
    let limit = limit.unwrap_or(10).clamp(1, 50);
    let results: serde_json::Value = client
        .get_json(&format!(
            "/api/search/{workspace}?query={}&limit={limit}",
            crate::enc(query)
        ))
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "workspace_id": workspace,
        "query": query,
        "results": results,
    }))
}
