use ira_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "hass_get_state".into(),
        description: "Lee el estado de una entidad de Home Assistant.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "string" }
            },
            "required": ["entity_id"]
        }),
    }
}

pub async fn run(client: &Client, entity_id: &str) -> Result<serde_json::Value, Error> {
    let state = client.get(&format!("/api/states/{entity_id}")).await?;
    Ok(serde_json::json!({
        "ok": true,
        "entity_id": state.get("entity_id"),
        "state": state.get("state"),
        "attributes": state.get("attributes"),
        "last_changed": state.get("last_changed"),
    }))
}
