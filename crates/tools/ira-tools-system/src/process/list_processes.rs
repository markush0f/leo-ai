use ira_llm::ToolSpec;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "list_processes".into(),
        description: "Lista procesos del sistema. Filtra por nombre con query.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Filtro opcional sobre el comando" },
                "limit": { "type": "integer", "description": "Máximo de resultados (por defecto 80)" }
            }
        }),
    }
}

pub async fn run(query: Option<&str>, limit: Option<u64>) -> Result<serde_json::Value, Error> {
    let query = query.map(|s| s.to_lowercase());
    let limit = limit.unwrap_or(80).clamp(1, 400) as usize;
    tokio::task::spawn_blocking(move || list_sync(query.as_deref(), limit))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
}

fn list_sync(query: Option<&str>, limit: usize) -> Result<serde_json::Value, Error> {
    let mut out = Vec::new();
    let rd =
        std::fs::read_dir("/proc").map_err(|_| Error::msg("solo disponible en Linux (/proc)"))?;
    for ent in rd {
        let ent = match ent {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = ent.file_name();
        let pid: i32 = match name.to_str().and_then(|s| s.parse().ok()) {
            Some(pid) => pid,
            None => continue,
        };
        let cmdline = read_cmdline(pid);
        let comm = read_comm(pid).unwrap_or_default();
        let hay = format!("{comm} {cmdline}").to_lowercase();
        if let Some(q) = query {
            if !hay.contains(q) {
                continue;
            }
        }
        out.push(serde_json::json!({
            "pid": pid,
            "name": comm,
            "cmd": crate::error::clip(&cmdline, 200),
        }));
        if out.len() >= limit {
            break;
        }
    }
    Ok(serde_json::json!({
        "ok": true,
        "count": out.len(),
        "processes": out,
    }))
}

pub(crate) fn read_cmdline(pid: i32) -> String {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    let joined = raw
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        read_comm(pid).unwrap_or_default()
    } else {
        joined
    }
}

pub(crate) fn read_comm(pid: i32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_string())
}
