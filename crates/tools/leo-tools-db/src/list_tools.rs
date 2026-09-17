use leo_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "db_list_tools".into(),
        description: "Lista las herramientas de base de datos que expone MCP Toolbox (nombre, descripción y parámetros).".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub async fn run(client: &Client) -> Result<serde_json::Value, Error> {
    let tools = client.list_tools().await?;
    let items: Vec<serde_json::Value> = tools
        .into_iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "ok": true,
        "count": items.len(),
        "tools": items,
    }))
}
