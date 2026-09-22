use ira_llm::ToolSpec;

use crate::client::{Client, calendar_url};
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "google_delete_event".into(),
        description: "Borra un evento de Google Calendar.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "calendar_id": { "type": "string" },
                "event_id": { "type": "string" }
            },
            "required": ["event_id"]
        }),
    }
}

pub async fn run(
    client: &Client,
    calendar_id: Option<&str>,
    event_id: &str,
) -> Result<serde_json::Value, Error> {
    client
        .delete(&calendar_url(
            calendar_id.unwrap_or("primary"),
            &format!("/events/{event_id}"),
        ))
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "event_id": event_id,
    }))
}
