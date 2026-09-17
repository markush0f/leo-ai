use leo_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "db_database_overview".into(),
        description: "Resumen del estado del servidor de base de datos vía MCP Toolbox.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub async fn run(client: &Client) -> Result<serde_json::Value, Error> {
    crate::invoke::run(client, "database_overview", serde_json::json!({})).await
}
