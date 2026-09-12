use leo_llm::ToolSpec;

use crate::client::{Client, repo_path};
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "github_list_pull_requests".into(),
        description: "Lista pull requests de un repositorio.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "owner": { "type": "string" },
                "repo": { "type": "string" },
                "state": { "type": "string", "description": "open, closed o all" },
                "limit": { "type": "integer" }
            },
            "required": ["owner"]
        }),
    }
}

pub async fn run(
    client: &Client,
    owner: &str,
    repo: &str,
    state: Option<&str>,
    limit: Option<u64>,
) -> Result<serde_json::Value, Error> {
    let per = limit.unwrap_or(20).clamp(1, 50);
    let state = state.unwrap_or("open");
    let items = client
        .get(&format!(
            "{}/pulls?state={state}&per_page={per}",
            repo_path(owner, repo)?
        ))
        .await?;
    let list: Vec<serde_json::Value> = items
        .as_array()
        .into_iter()
        .flatten()
        .map(|i| {
            serde_json::json!({
                "number": i.get("number"),
                "title": i.get("title"),
                "state": i.get("state"),
                "head": i.get("head").and_then(|h| h.get("ref")),
                "base": i.get("base").and_then(|h| h.get("ref")),
                "url": i.get("html_url"),
            })
        })
        .collect();
    Ok(serde_json::json!({
        "ok": true,
        "count": list.len(),
        "pull_requests": list,
    }))
}
