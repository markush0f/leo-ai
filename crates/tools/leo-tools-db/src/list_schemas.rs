use leo_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "db_list_schemas".into(),
        description: "Lista los esquemas de la base configurada en MCP Toolbox.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub async fn run(client: &Client) -> Result<serde_json::Value, Error> {
    crate::invoke::run(client, "list_schemas", serde_json::json!({})).await
}
