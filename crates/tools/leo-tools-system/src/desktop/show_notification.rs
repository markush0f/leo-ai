use leo_llm::ToolSpec;
use tokio::process::Command;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "show_notification".into(),
        description: "Muestra una notificación de escritorio (notify-send).".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Título" },
                "body": { "type": "string", "description": "Cuerpo" }
            },
            "required": ["title"]
        }),
    }
}

pub async fn run(title: &str, body: Option<&str>) -> Result<serde_json::Value, Error> {
    let mut cmd = Command::new("notify-send");
    cmd.arg(title);
    if let Some(body) = body.filter(|s| !s.is_empty()) {
        cmd.arg(body);
    }
    let status = cmd
        .status()
        .await
        .map_err(|_| Error::msg("no se encontró notify-send; instala libnotify"))?;
    if !status.success() {
        return Err(Error::msg("notify-send falló"));
    }
    Ok(serde_json::json!({ "ok": true, "title": title }))
}
