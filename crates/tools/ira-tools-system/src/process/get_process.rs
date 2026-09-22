use ira_llm::ToolSpec;

use super::list_processes::{read_cmdline, read_comm};
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "get_process".into(),
        description: "Detalles de un proceso por pid.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "pid": { "type": "integer", "description": "PID" }
            },
            "required": ["pid"]
        }),
    }
}

pub async fn run(pid: i32) -> Result<serde_json::Value, Error> {
    tokio::task::spawn_blocking(move || get_sync(pid))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
}

fn get_sync(pid: i32) -> Result<serde_json::Value, Error> {
    let status_path = format!("/proc/{pid}/status");
    let status = std::fs::read_to_string(&status_path)
        .map_err(|_| Error::msg(format!("no existe el pid {pid}")))?;
    let mut state = None;
    let mut ppid = None;
    let mut rss = None;
    for line in status.lines() {
        if let Some(v) = line.strip_prefix("State:") {
            state = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("PPid:") {
            ppid = v.trim().parse::<i32>().ok();
        } else if let Some(v) = line.strip_prefix("VmRSS:") {
            rss = Some(v.trim().to_string());
        }
    }
    Ok(serde_json::json!({
        "ok": true,
        "pid": pid,
        "name": read_comm(pid),
        "cmd": read_cmdline(pid),
        "state": state,
        "ppid": ppid,
        "rss": rss,
    }))
}
