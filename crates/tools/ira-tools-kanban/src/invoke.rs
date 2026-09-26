use ira_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "kanban_invoke".into(),
        description: "Invoca una herramienta del MCP de Veritas Kanban por su nombre remoto (list_tasks, create_task, update_task). Usa kanban_list_tools para ver el catálogo. No uses el prefijo kanban_.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "tool": { "type": "string", "description": "Nombre remoto en el MCP de Veritas Kanban" },
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
