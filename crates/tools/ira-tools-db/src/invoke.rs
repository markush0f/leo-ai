use ira_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "db_invoke".into(),
        description: "Invoca una herramienta de MCP Toolbox por su nombre. Usa db_list_tools para ver las disponibles.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "tool": { "type": "string", "description": "Nombre de la herramienta en Toolbox" },
                "arguments": { "type": "object", "description": "Argumentos JSON de esa herramienta" }
            },
            "required": ["tool"]
        }),
    }
}

pub async fn run(
    client: &Client,
    name: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    let result = client.invoke(name, arguments).await?;
    Ok(serde_json::json!({
        "ok": true,
        "tool": name,
        "result": result,
    }))
}
