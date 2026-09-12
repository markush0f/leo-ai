use leo_llm::ToolSpec;
use serde::Deserialize;

use crate::client::Client;
use crate::error::Error;
use crate::markdown;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "appflowy_create_page".into(),
        description: "Crea una página en AppFlowy con título y cuerpo en markdown.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Título de la página" },
                "markdown": { "type": "string", "description": "Contenido en markdown" },
                "parent_view_id": { "type": "string", "description": "view_id del padre (opcional)" }
            },
            "required": ["title"]
        }),
    }
}

pub fn spec_write() -> ToolSpec {
    let mut spec = spec();
    spec.name = "appflowy_write".into();
    spec.description = "Crea una página nueva en AppFlowy con el título y el cuerpo en markdown. \
Usa esto cuando el usuario pida guardar, anotar o escribir algo en AppFlowy."
        .into();
    spec
}

pub async fn run(
    client: &Client,
    title: &str,
    markdown_body: &str,
    parent_view_id: Option<&str>,
) -> Result<serde_json::Value, Error> {
    let workspace = client.workspace_id().await?;
    let parent = match parent_view_id.filter(|s| !s.is_empty()) {
        Some(id) => id.to_string(),
        None => match client.parent_view_id() {
            Some(id) => id.to_string(),
            None => client.default_parent(&workspace).await?,
        },
    };
    let created: Page = client
        .post_json(
            &format!("/api/workspace/{workspace}/page-view"),
            &serde_json::json!({
                "parent_view_id": parent,
                "layout": 0,
                "name": title,
            }),
        )
        .await?;
    if !markdown_body.trim().is_empty() {
        let blocks = markdown::to_blocks(markdown_body);
        client
            .post_json::<serde_json::Value>(
                &format!(
                    "/api/workspace/{workspace}/page-view/{}/append-block",
                    created.view_id
                ),
                &serde_json::json!({ "blocks": blocks }),
            )
            .await?;
    }
    Ok(serde_json::json!({
        "ok": true,
        "view_id": created.view_id,
        "title": title,
        "workspace_id": workspace,
    }))
}

#[derive(Deserialize)]
struct Page {
    view_id: String,
}
