use std::path::Path;

use ira_llm::ToolSpec;
use tokio::io::AsyncWriteExt;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "write_file".into(),
        description: "Escribe (o añade) texto a un archivo. Crea directorios padre si faltan."
            .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Ruta del archivo" },
                "content": { "type": "string", "description": "Contenido a escribir" },
                "append": { "type": "boolean", "description": "Si true, añade al final en vez de sobrescribir" }
            },
            "required": ["path", "content"]
        }),
    }
}

pub async fn run(path: &Path, content: &str, append: bool) -> Result<serde_json::Value, Error> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    if append {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        file.write_all(content.as_bytes()).await?;
    } else {
        tokio::fs::write(path, content.as_bytes()).await?;
    }
    Ok(serde_json::json!({
        "ok": true,
        "path": path.display().to_string(),
        "bytes": content.len(),
        "append": append,
    }))
}
