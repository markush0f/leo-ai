use ira_llm::ToolSpec;

use crate::client::{Client, calendar_url};
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "google_get_event".into(),
        description: "Lee un evento de Google Calendar por id.".into(),
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
    let event = client
        .get(&calendar_url(
            calendar_id.unwrap_or("primary"),
            &format!("/events/{event_id}"),
        ))
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "id": event.get("id"),
        "summary": event.get("summary"),
        "description": event.get("description"),
        "start": event.get("start"),
        "end": event.get("end"),
        "location": event.get("location"),
        "htmlLink": event.get("htmlLink"),
    }))
}
