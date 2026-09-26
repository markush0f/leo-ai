use ira_llm::ToolSpec;

use crate::client::Client;
use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "whatsapp_send".into(),
        description: "Envía un WhatsApp al número configurado de Markus vía CallMeBot. \
Úsalo cuando pida avisarle, recordarle o mandarle un mensaje al móvil. \
No acepta otro destinatario y no recibe respuesta."
            .into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Texto del WhatsApp" }
            },
            "required": ["text"]
        }),
    }
}

pub async fn run(client: &Client, text: &str) -> Result<serde_json::Value, Error> {
    client.send(text).await?;
    Ok(serde_json::json!({ "ok": true }))
}
