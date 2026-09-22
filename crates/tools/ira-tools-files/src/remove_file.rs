use std::path::Path;

use ira_llm::ToolSpec;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "remove_file".into(),
        description: "Borra un archivo. Con recursive=true borra un directorio entero.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Ruta a borrar" },
                "recursive": { "type": "boolean", "description": "Borrar directorio y contenido" }
            },
            "required": ["path"]
        }),
    }
}

pub async fn run(path: &Path, recursive: bool) -> Result<serde_json::Value, Error> {
    let meta = tokio::fs::metadata(path).await?;
    if meta.is_dir() {
        if !recursive {
            return Err(Error::msg(
                "es un directorio; pasa recursive=true para borrarlo",
            ));
        }
        tokio::fs::remove_dir_all(path).await?;
    } else {
        tokio::fs::remove_file(path).await?;
    }
    Ok(serde_json::json!({
        "ok": true,
        "path": path.display().to_string(),
        "recursive": recursive,
    }))
}
