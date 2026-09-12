use leo_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "appflowy_delete_page".into(),
        description: "Mueve una página de AppFlowy a la papelera.".into(),
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
    client
        .post_json::<serde_json::Value>(
            &format!("/api/workspace/{workspace}/page-view/{view_id}/move-to-trash"),
            &serde_json::json!({}),
        )
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "view_id": view_id,
        "trashed": true,
    }))
}
