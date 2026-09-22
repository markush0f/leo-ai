use std::path::Path;

use ira_llm::ToolSpec;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "search_files".into(),
        description: "Busca archivos por nombre (subcadena o glob sencillo con *).".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Nombre o patrón (* admite comodín)" },
                "path": { "type": "string", "description": "Directorio raíz (por defecto .)" },
                "max_results": { "type": "integer", "description": "Máximo de resultados (por defecto 100)" }
            },
            "required": ["query"]
        }),
    }
}

pub async fn run(
    root: &Path,
    query: &str,
    max_results: Option<u64>,
) -> Result<serde_json::Value, Error> {
    let root = root.to_path_buf();
    let query = query.to_string();
    let max = max_results.unwrap_or(100).clamp(1, 500) as usize;
    tokio::task::spawn_blocking(move || search_sync(&root, &query, max))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
}

fn search_sync(root: &Path, query: &str, max: usize) -> Result<serde_json::Value, Error> {
    let mut matches = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut visited = 0usize;
    while let Some(dir) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for ent in rd {
            let ent = match ent {
                Ok(e) => e,
                Err(_) => continue,
            };
            visited += 1;
            if visited > 8000 {
                break;
            }
            let path = ent.path();
            let name = ent.file_name().to_string_lossy().into_owned();
            if glob_match(&name, query) {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                matches.push(rel.display().to_string());
                if matches.len() >= max {
                    break;
                }
            }
            if path.is_dir() {
                stack.push(path);
            }
        }
        if matches.len() >= max || visited > 8000 {
            break;
        }
    }
    Ok(serde_json::json!({
        "ok": true,
        "query": query,
        "count": matches.len(),
        "matches": matches,
    }))
}

fn glob_match(name: &str, pat: &str) -> bool {
    let name = name.to_lowercase();
    let pat = pat.to_lowercase();
    if !pat.contains('*') {
        return name.contains(&pat);
    }
    let parts: Vec<&str> = pat.split('*').collect();
    let mut rest = name.as_str();
    if let Some(first) = parts.first() {
        if !first.is_empty() {
            if let Some(stripped) = rest.strip_prefix(first) {
                rest = stripped;
            } else {
                return false;
            }
        }
    }
    if let Some(last) = parts.last() {
        if !last.is_empty() && parts.len() > 1 {
            if let Some(stripped) = rest.strip_suffix(last) {
                rest = stripped;
            } else {
                return false;
            }
        }
    }
    for part in parts.iter().skip(1).take(parts.len().saturating_sub(2)) {
        if part.is_empty() {
            continue;
        }
        if let Some(idx) = rest.find(part) {
            rest = &rest[idx + part.len()..];
        } else {
            return false;
        }
    }
    true
}
