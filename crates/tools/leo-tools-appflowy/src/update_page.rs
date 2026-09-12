use leo_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;
use crate::markdown;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "appflowy_update_page".into(),
        description: "Actualiza una página: renombra y/o añade markdown al final.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "view_id": { "type": "string" },
                "title": { "type": "string", "description": "Nuevo título" },
                "markdown": { "type": "string", "description": "Markdown a añadir al final" }
            },
            "required": ["view_id"]
        }),
    }
}

pub async fn run(
    client: &Client,
    view_id: &str,
    title: Option<&str>,
    markdown_body: Option<&str>,
) -> Result<serde_json::Value, Error> {
    let workspace = client.workspace_id().await?;
    if let Some(title) = title.filter(|s| !s.is_empty()) {
        client
            .post_json::<serde_json::Value>(
                &format!("/api/workspace/{workspace}/page-view/{view_id}/update-name"),
                &serde_json::json!({ "name": title }),
            )
            .await?;
    }
    if let Some(body) = markdown_body.filter(|s| !s.trim().is_empty()) {
        let blocks = markdown::to_blocks(body);
        client
            .post_json::<serde_json::Value>(
                &format!("/api/workspace/{workspace}/page-view/{view_id}/append-block"),
                &serde_json::json!({ "blocks": blocks }),
            )
            .await?;
    }
    Ok(serde_json::json!({
        "ok": true,
        "view_id": view_id,
        "workspace_id": workspace,
    }))
}
