use std::path::Path;

use ira_llm::ToolSpec;
use tokio::io::AsyncReadExt;

use crate::error::Error;

const DEFAULT_MAX: u64 = 256 * 1024;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "read_file".into(),
        description:
            "Lee el contenido de un archivo de texto. Rutas relativas al directorio de trabajo."
                .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Ruta del archivo" },
                "max_bytes": { "type": "integer", "description": "Tope de bytes a leer (por defecto 262144)" }
            },
            "required": ["path"]
        }),
    }
}

pub async fn run(path: &Path, max_bytes: Option<u64>) -> Result<serde_json::Value, Error> {
    let meta = tokio::fs::metadata(path).await?;
    if meta.is_dir() {
        return Err(Error::msg(format!("{} es un directorio", path.display())));
    }
    let max = max_bytes.unwrap_or(DEFAULT_MAX).max(1);
    let file = tokio::fs::File::open(path).await?;
    let mut buf = Vec::new();
    file.take(max + 1).read_to_end(&mut buf).await?;
    let truncated = buf.len() as u64 > max;
    if truncated {
        buf.truncate(max as usize);
    }
    let lossy = String::from_utf8_lossy(&buf);
    Ok(serde_json::json!({
        "ok": true,
        "path": path.display().to_string(),
        "bytes": buf.len(),
        "truncated": truncated,
        "content": lossy,
    }))
}
