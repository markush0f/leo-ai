use ira_llm::ToolSpec;

use crate::client::{Client, calendar_url};
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "google_list_events".into(),
        description: "Lista eventos de un calendario. Fechas en RFC3339.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "calendar_id": { "type": "string", "description": "Por defecto primary" },
                "time_min": { "type": "string", "description": "Inicio RFC3339" },
                "time_max": { "type": "string", "description": "Fin RFC3339" },
                "limit": { "type": "integer" },
                "query": { "type": "string" }
            }
        }),
    }
}

pub async fn run(
    client: &Client,
    calendar_id: Option<&str>,
    time_min: Option<&str>,
    time_max: Option<&str>,
    query: Option<&str>,
    limit: Option<u64>,
) -> Result<serde_json::Value, Error> {
    let mut url = calendar_url(
        calendar_id.unwrap_or("primary"),
        "/events?singleEvents=true&orderBy=startTime",
    );
    url.push_str(&format!("&maxResults={}", limit.unwrap_or(20).clamp(1, 50)));
    if let Some(t) = time_min {
        url.push_str(&format!("&timeMin={}", crate::client_enc(t)));
    }
    if let Some(t) = time_max {
        url.push_str(&format!("&timeMax={}", crate::client_enc(t)));
    }
    if let Some(q) = query {
        url.push_str(&format!("&q={}", crate::client_enc(q)));
    }
    let v = client.get(&url).await?;
    let items: Vec<serde_json::Value> = v
        .get("items")
        .and_then(|i| i.as_array())
        .into_iter()
        .flatten()
        .map(|e| {
            serde_json::json!({
                "id": e.get("id"),
                "summary": e.get("summary"),
                "start": e.get("start"),
                "end": e.get("end"),
                "htmlLink": e.get("htmlLink"),
            })
        })
        .collect();
    Ok(serde_json::json!({ "ok": true, "count": items.len(), "events": items }))
}
