use ira_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "hass_call_service".into(),
        description: "Llama a un servicio de Home Assistant (light.turn_on, switch.toggle, …)."
            .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "domain": { "type": "string", "description": "light, switch, climate, …" },
                "service": { "type": "string", "description": "turn_on, turn_off, toggle, …" },
                "entity_id": { "type": "string" },
                "data": { "type": "object", "description": "Datos extra del servicio" }
            },
            "required": ["domain", "service"]
        }),
    }
}

pub async fn run(
    client: &Client,
    domain: &str,
    service: &str,
    entity_id: Option<&str>,
    data: Option<&serde_json::Value>,
) -> Result<serde_json::Value, Error> {
    let mut body = match data {
        Some(serde_json::Value::Object(map)) => serde_json::Value::Object(map.clone()),
        _ => serde_json::json!({}),
    };
    if let Some(id) = entity_id {
        body["entity_id"] = id.into();
    }
    let result = client
        .post(&format!("/api/services/{domain}/{service}"), &body)
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "domain": domain,
        "service": service,
        "result": result,
    }))
}
