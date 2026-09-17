use leo_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "db_execute_sql".into(),
        description: "Ejecuta una sentencia SQL en la base configurada en MCP Toolbox.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "sql": { "type": "string", "description": "Sentencia SQL" }
            },
            "required": ["sql"]
        }),
    }
}

pub async fn run(client: &Client, sql: &str) -> Result<serde_json::Value, Error> {
    crate::invoke::run(client, "execute_sql", serde_json::json!({ "sql": sql })).await
}
