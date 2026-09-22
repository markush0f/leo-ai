use ira_llm::ToolSpec;
use tokio::process::Command;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "open_application".into(),
        description: "Lanza una aplicación de escritorio por nombre de comando o .desktop.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Comando o id de aplicación (firefox, code, …)" }
            },
            "required": ["name"]
        }),
    }
}

pub async fn run(name: &str) -> Result<serde_json::Value, Error> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\0') {
        return Err(Error::msg("nombre de aplicación no válido"));
    }
    let gtk = Command::new("gtk-launch")
        .arg(name)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
    if let Ok(status) = gtk {
        if status.success() {
            return Ok(serde_json::json!({ "ok": true, "name": name, "via": "gtk-launch" }));
        }
    }
    let status = Command::new(name)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| Error::msg(format!("no se pudo lanzar {name}")))?;
    drop(status);
    Ok(serde_json::json!({ "ok": true, "name": name, "via": "exec" }))
}
