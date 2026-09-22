use ira_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "db_get_query_plan".into(),
        description: "Obtiene el plan de ejecución de una sentencia SQL vía MCP Toolbox.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "sql": { "type": "string", "description": "Sentencia a explicar" }
            },
            "required": ["sql"]
        }),
    }
}

pub async fn run(client: &Client, sql: &str) -> Result<serde_json::Value, Error> {
    crate::invoke::run(client, "get_query_plan", serde_json::json!({ "sql": sql })).await
}
