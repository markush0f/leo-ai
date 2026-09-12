use leo_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "google_list_calendars".into(),
        description: "Lista los calendarios de la cuenta de Google.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub async fn run(client: &Client) -> Result<serde_json::Value, Error> {
    let v = client
        .get("https://www.googleapis.com/calendar/v3/users/me/calendarList")
        .await?;
    let items: Vec<serde_json::Value> = v
        .get("items")
        .and_then(|i| i.as_array())
        .into_iter()
        .flatten()
        .map(|c| {
            serde_json::json!({
                "id": c.get("id"),
                "summary": c.get("summary"),
                "primary": c.get("primary"),
                "timeZone": c.get("timeZone"),
            })
        })
        .collect();
    Ok(serde_json::json!({ "ok": true, "count": items.len(), "calendars": items }))
}
