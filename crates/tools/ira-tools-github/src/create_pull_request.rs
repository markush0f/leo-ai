use ira_llm::ToolSpec;

use crate::client::{Client, repo_path};
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "github_create_pull_request".into(),
        description: "Crea un pull request. `head` es la rama origen, `base` la destino.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "owner": { "type": "string" },
                "repo": { "type": "string" },
                "title": { "type": "string" },
                "head": { "type": "string", "description": "Rama origen" },
                "base": { "type": "string", "description": "Rama destino (por defecto main)" },
                "body": { "type": "string" }
            },
            "required": ["owner", "title", "head"]
        }),
    }
}

pub async fn run(
    client: &Client,
    owner: &str,
    repo: &str,
    title: &str,
    head: &str,
    base: Option<&str>,
    body: Option<&str>,
) -> Result<serde_json::Value, Error> {
    let payload = serde_json::json!({
        "title": title,
        "head": head,
        "base": base.unwrap_or("main"),
        "body": body.unwrap_or(""),
    });
    let created = client
        .post(&format!("{}/pulls", repo_path(owner, repo)?), &payload)
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "number": created.get("number"),
        "url": created.get("html_url"),
        "title": created.get("title"),
    }))
}
