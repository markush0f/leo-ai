use ira_llm::ToolSpec;

use crate::client::{Client, calendar_url};
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "google_create_event".into(),
        description: "Crea un evento. `start` y `end` en RFC3339, o `date` para todo el día."
            .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "calendar_id": { "type": "string" },
                "summary": { "type": "string", "description": "Título" },
                "description": { "type": "string" },
                "start": { "type": "string", "description": "Inicio RFC3339" },
                "end": { "type": "string", "description": "Fin RFC3339" },
                "date": { "type": "string", "description": "YYYY-MM-DD si es todo el día" },
                "location": { "type": "string" }
            },
            "required": ["summary"]
        }),
    }
}

pub async fn run(
    client: &Client,
    calendar_id: Option<&str>,
    summary: &str,
    description: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    date: Option<&str>,
    location: Option<&str>,
) -> Result<serde_json::Value, Error> {
    let (start_obj, end_obj) = times(start, end, date)?;
    let mut body = serde_json::json!({
        "summary": summary,
        "start": start_obj,
        "end": end_obj,
    });
    if let Some(d) = description {
        body["description"] = d.into();
    }
    if let Some(l) = location {
        body["location"] = l.into();
    }
    let created = client
        .post(
            &calendar_url(calendar_id.unwrap_or("primary"), "/events"),
            &body,
        )
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "id": created.get("id"),
        "summary": created.get("summary"),
        "htmlLink": created.get("htmlLink"),
    }))
}

pub(crate) fn times(
    start: Option<&str>,
    end: Option<&str>,
    date: Option<&str>,
) -> Result<(serde_json::Value, serde_json::Value), Error> {
    if let Some(date) = date {
        return Ok((
            serde_json::json!({ "date": date }),
            serde_json::json!({ "date": date }),
        ));
    }
    let start = start.ok_or_else(|| Error::msg("falta start o date"))?;
    let end = end.unwrap_or(start);
    Ok((
        serde_json::json!({ "dateTime": start }),
        serde_json::json!({ "dateTime": end }),
    ))
}
