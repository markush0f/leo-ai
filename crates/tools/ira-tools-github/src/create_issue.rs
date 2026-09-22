use ira_llm::ToolSpec;

use crate::client::{Client, repo_path};
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "github_create_issue".into(),
        description: "Crea un issue en un repositorio de GitHub.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "owner": { "type": "string", "description": "Dueño o `owner/repo`" },
                "repo": { "type": "string", "description": "Repositorio" },
                "title": { "type": "string" },
                "body": { "type": "string" },
                "labels": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["owner", "title"]
        }),
    }
}

pub async fn run(
    client: &Client,
    owner: &str,
    repo: &str,
    title: &str,
    body: Option<&str>,
    labels: &[String],
) -> Result<serde_json::Value, Error> {
    let mut payload = serde_json::json!({ "title": title });
    if let Some(body) = body {
        payload["body"] = body.into();
    }
    if !labels.is_empty() {
        payload["labels"] = labels.into();
    }
    let created = client
        .post(&format!("{}/issues", repo_path(owner, repo)?), &payload)
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "number": created.get("number"),
        "url": created.get("html_url"),
        "title": created.get("title"),
    }))
}
