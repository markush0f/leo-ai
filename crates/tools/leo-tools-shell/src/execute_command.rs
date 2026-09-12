use std::path::Path;
use std::time::Duration;

use leo_llm::ToolSpec;
use tokio::process::Command;

use crate::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "execute_command".into(),
        description: "Ejecuta un programa. Pasa `program` y `args`, o `command` para `sh -c`."
            .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "program": { "type": "string", "description": "Ejecutable" },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Argumentos"
                },
                "command": { "type": "string", "description": "Línea para sh -c si no hay program" },
                "cwd": { "type": "string", "description": "Directorio de trabajo" },
                "timeout_secs": { "type": "integer", "description": "Tope en segundos (por defecto 30)" }
            }
        }),
    }
}

pub async fn run(
    program: Option<&str>,
    args: &[String],
    command: Option<&str>,
    cwd: Option<&Path>,
    timeout_secs: Option<u64>,
) -> Result<serde_json::Value, Error> {
    let timeout = Duration::from_secs(timeout_secs.unwrap_or(30).clamp(1, 300));
    let mut cmd = if let Some(program) = program.filter(|s| !s.is_empty()) {
        let mut c = Command::new(program);
        c.args(args);
        c
    } else if let Some(command) = command.filter(|s| !s.is_empty()) {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    } else {
        return Err(Error::msg("falta program o command"));
    };
    crate::apply_cwd(&mut cmd, cwd);
    crate::run_command(cmd, timeout).await
}
