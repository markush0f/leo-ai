use ira_llm::ToolSpec;

use crate::client::{Client, repo_path};
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "github_get_issue".into(),
        description: "Lee un issue o pull request por número.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "owner": { "type": "string" },
                "repo": { "type": "string" },
                "number": { "type": "integer" }
            },
            "required": ["owner", "number"]
        }),
    }
}

pub async fn run(
    client: &Client,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<serde_json::Value, Error> {
    let issue = client
        .get(&format!("{}/issues/{number}", repo_path(owner, repo)?))
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "number": issue.get("number"),
        "title": issue.get("title"),
        "state": issue.get("state"),
        "body": issue.get("body"),
        "url": issue.get("html_url"),
        "user": issue.get("user").and_then(|u| u.get("login")),
        "labels": issue.get("labels"),
    }))
}
