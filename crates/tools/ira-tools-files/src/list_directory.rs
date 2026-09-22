use std::path::Path;

use ira_llm::ToolSpec;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "list_directory".into(),
        description: "Lista archivos y carpetas de un directorio.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directorio a listar (por defecto .)" },
                "recursive": { "type": "boolean", "description": "Recorrer subcarpetas" }
            }
        }),
    }
}

pub async fn run(path: &Path, recursive: bool) -> Result<serde_json::Value, Error> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || list_sync(&path, recursive))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
}

fn list_sync(path: &Path, recursive: bool) -> Result<serde_json::Value, Error> {
    let mut entries = Vec::new();
    if recursive {
        walk(path, path, &mut entries, 0)?;
    } else {
        let rd = std::fs::read_dir(path)?;
        for ent in rd {
            let ent = ent?;
            entries.push(describe(&ent.path(), path)?);
        }
    }
    entries.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    Ok(serde_json::json!({
        "ok": true,
        "path": path.display().to_string(),
        "count": entries.len(),
        "entries": entries,
    }))
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<serde_json::Value>, depth: u8) -> Result<(), Error> {
    if depth > 12 || out.len() >= 500 {
        return Ok(());
    }
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(()),
    };
    for ent in rd {
        let ent = match ent {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = ent.path();
        out.push(describe(&p, root)?);
        if p.is_dir() {
            walk(root, &p, out, depth + 1)?;
        }
        if out.len() >= 500 {
            break;
        }
    }
    Ok(())
}

fn describe(path: &Path, root: &Path) -> Result<serde_json::Value, Error> {
    let meta = std::fs::metadata(path)?;
    let rel = path.strip_prefix(root).unwrap_or(path);
    Ok(serde_json::json!({
        "path": rel.display().to_string(),
        "kind": if meta.is_dir() { "dir" } else { "file" },
        "size": meta.len(),
    }))
}
