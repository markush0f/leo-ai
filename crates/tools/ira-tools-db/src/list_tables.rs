use ira_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "db_list_tables".into(),
        description: "Lista tablas y su esquema en la base de MCP Toolbox.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "table_names": {
                    "type": "string",
                    "description": "Nombres separados por coma; vacío lista todas"
                }
            }
        }),
    }
}

pub async fn run(client: &Client, table_names: Option<&str>) -> Result<serde_json::Value, Error> {
    let mut args = serde_json::Map::new();
    if let Some(names) = table_names.filter(|s| !s.is_empty()) {
        args.insert(
            "table_names".into(),
            serde_json::Value::String(names.into()),
        );
    }
    crate::invoke::run(client, "list_tables", serde_json::Value::Object(args)).await
}
