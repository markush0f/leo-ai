use std::path::Path;
use std::time::Duration;

use ira_llm::ToolSpec;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "execute_script".into(),
        description: "Ejecuta un script (varias líneas) con sh o el intérprete indicado.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "script": { "type": "string", "description": "Cuerpo del script" },
                "interpreter": { "type": "string", "description": "Intérprete (por defecto sh)" },
                "cwd": { "type": "string", "description": "Directorio de trabajo" },
                "timeout_secs": { "type": "integer", "description": "Tope en segundos (por defecto 30)" }
            },
            "required": ["script"]
        }),
    }
}

pub async fn run(
    script: &str,
    interpreter: Option<&str>,
    cwd: Option<&Path>,
    timeout_secs: Option<u64>,
) -> Result<serde_json::Value, Error> {
    if script.trim().is_empty() {
        return Err(Error::msg("script vacío"));
    }
    let timeout = Duration::from_secs(timeout_secs.unwrap_or(30).clamp(1, 300));
    let interpreter = interpreter.unwrap_or("sh");
    let mut cmd = Command::new(interpreter);
    cmd.arg("-s")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    crate::apply_cwd(&mut cmd, cwd);
    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(script.as_bytes()).await?;
    }
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => {
            return Err(Error::msg(format!(
                "tiempo agotado tras {}s",
                timeout.as_secs()
            )));
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(serde_json::json!({
        "ok": output.status.success(),
        "exit_code": output.status.code(),
        "stdout": stdout,
        "stderr": stderr,
    }))
}
