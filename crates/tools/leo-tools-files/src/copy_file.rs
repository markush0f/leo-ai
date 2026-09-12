use std::path::Path;

use leo_llm::ToolSpec;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "copy_file".into(),
        description: "Copia un archivo a otra ruta.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "from": { "type": "string", "description": "Ruta origen" },
                "to": { "type": "string", "description": "Ruta destino" }
            },
            "required": ["from", "to"]
        }),
    }
}

pub async fn run(from: &Path, to: &Path) -> Result<serde_json::Value, Error> {
    if let Some(parent) = to.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    tokio::fs::copy(from, to).await?;
    Ok(serde_json::json!({
        "ok": true,
        "from": from.display().to_string(),
        "to": to.display().to_string(),
    }))
}
