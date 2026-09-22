use ira_llm::ToolSpec;

use crate::client::{Client, calendar_url};
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "google_update_event".into(),
        description: "Actualiza un evento (título, descripción, fechas).".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "calendar_id": { "type": "string" },
                "event_id": { "type": "string" },
                "summary": { "type": "string" },
                "description": { "type": "string" },
                "start": { "type": "string" },
                "end": { "type": "string" },
                "date": { "type": "string" },
                "location": { "type": "string" }
            },
            "required": ["event_id"]
        }),
    }
}

pub async fn run(
    client: &Client,
    calendar_id: Option<&str>,
    event_id: &str,
    summary: Option<&str>,
    description: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    date: Option<&str>,
    location: Option<&str>,
) -> Result<serde_json::Value, Error> {
    let mut body = serde_json::json!({});
    if let Some(s) = summary {
        body["summary"] = s.into();
    }
    if let Some(d) = description {
        body["description"] = d.into();
    }
    if let Some(l) = location {
        body["location"] = l.into();
    }
    if start.is_some() || end.is_some() || date.is_some() {
        let (s, e) = crate::create_event::times(start, end, date)?;
        body["start"] = s;
        body["end"] = e;
    }
    let updated = client
        .patch(
            &calendar_url(
                calendar_id.unwrap_or("primary"),
                &format!("/events/{event_id}"),
            ),
            &body,
        )
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "id": updated.get("id"),
        "summary": updated.get("summary"),
        "htmlLink": updated.get("htmlLink"),
    }))
}
