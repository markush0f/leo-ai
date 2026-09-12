use leo_llm::ToolSpec;
use tokio::process::Command;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "kill_process".into(),
        description: "Envía una señal a un proceso (por defecto SIGTERM).".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "pid": { "type": "integer", "description": "PID" },
                "signal": { "type": "string", "description": "Señal: TERM, KILL, INT, HUP (por defecto TERM)" }
            },
            "required": ["pid"]
        }),
    }
}

pub async fn run(pid: i32, signal: Option<&str>) -> Result<serde_json::Value, Error> {
    if pid <= 1 {
        return Err(Error::msg("pid no permitido"));
    }
    let signal = normalize(signal.unwrap_or("TERM"));
    let status = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(pid.to_string())
        .status()
        .await?;
    if !status.success() {
        return Err(Error::msg(format!(
            "kill {pid} falló (código {:?})",
            status.code()
        )));
    }
    Ok(serde_json::json!({
        "ok": true,
        "pid": pid,
        "signal": signal,
    }))
}

fn normalize(s: &str) -> String {
    let t = s.trim().trim_start_matches("SIG").to_ascii_uppercase();
    match t.as_str() {
        "TERM" | "KILL" | "INT" | "HUP" | "USR1" | "USR2" | "QUIT" => t,
        other if other.chars().all(|c| c.is_ascii_digit()) => other.to_string(),
        _ => "TERM".into(),
    }
}
