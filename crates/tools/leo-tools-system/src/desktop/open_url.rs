use leo_llm::ToolSpec;
use tokio::process::Command;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "open_url".into(),
        description: "Abre una URL en el navegador (xdg-open).".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL" }
            },
            "required": ["url"]
        }),
    }
}

pub async fn run(url: &str) -> Result<serde_json::Value, Error> {
    if !(url.starts_with("http://") || url.starts_with("https://") || url.starts_with("file:")) {
        return Err(Error::msg("la url debe empezar por http(s):// o file:"));
    }
    let status = Command::new("xdg-open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map_err(|_| Error::msg("no se encontró xdg-open"))?;
    if !status.success() {
        return Err(Error::msg("xdg-open falló"));
    }
    Ok(serde_json::json!({ "ok": true, "url": url }))
}
