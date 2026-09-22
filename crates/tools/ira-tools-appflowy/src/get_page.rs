use ira_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "appflowy_get_page".into(),
        description: "Lee una página de AppFlowy por view_id.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "view_id": { "type": "string" }
            },
            "required": ["view_id"]
        }),
    }
}

pub async fn run(client: &Client, view_id: &str) -> Result<serde_json::Value, Error> {
    let workspace = client.workspace_id().await?;
    let page: serde_json::Value = client
        .get_json(&format!("/api/workspace/{workspace}/page-view/{view_id}"))
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "workspace_id": workspace,
        "page": page,
    }))
}
