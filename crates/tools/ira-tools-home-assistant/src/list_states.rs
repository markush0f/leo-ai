use ira_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "hass_list_states".into(),
        description: "Lista entidades de Home Assistant. Filtra por dominio (light, switch, …)."
            .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "domain": { "type": "string", "description": "Prefijo de entity_id, p.ej. light" },
                "limit": { "type": "integer" }
            }
        }),
    }
}

pub async fn run(
    client: &Client,
    domain: Option<&str>,
    limit: Option<u64>,
) -> Result<serde_json::Value, Error> {
    let states = client.get("/api/states").await?;
    let limit = limit.unwrap_or(80).clamp(1, 300) as usize;
    let prefix = domain.map(|d| format!("{d}."));
    let items: Vec<serde_json::Value> = states
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| match &prefix {
            Some(p) => s
                .get("entity_id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| id.starts_with(p)),
            None => true,
        })
        .take(limit)
        .map(|s| {
            serde_json::json!({
                "entity_id": s.get("entity_id"),
                "state": s.get("state"),
                "friendly_name": s.get("attributes").and_then(|a| a.get("friendly_name")),
            })
        })
        .collect();
    Ok(serde_json::json!({
        "ok": true,
        "count": items.len(),
        "states": items,
    }))
}
