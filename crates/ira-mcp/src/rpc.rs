use serde_json::{Value, json};

use crate::error::Error;

pub const PROTOCOL: &str = "2025-03-26";

#[derive(Debug, Clone)]
pub struct RemoteTool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

pub fn initialize_params() -> Value {
    json!({
        "protocolVersion": PROTOCOL,
        "capabilities": {},
        "clientInfo": { "name": "ira", "version": env!("CARGO_PKG_VERSION") },
    })
}

pub fn decode_rpc(content_type: &str, body: &str) -> Result<Value, Error> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    let payload = if content_type.contains("event-stream")
        || trimmed.starts_with("event:")
        || trimmed.starts_with("data:")
    {
        sse_data(trimmed).ok_or_else(|| Error::msg("respuesta SSE vacía"))?
    } else {
        trimmed.to_string()
    };
    Ok(serde_json::from_str(&payload)?)
}

pub fn sse_data(body: &str) -> Option<String> {
    let mut last = None;
    for line in body.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if !data.is_empty() && data != "[DONE]" {
                last = Some(data.to_string());
            }
        }
    }
    last
}

pub fn rpc_error(rpc: &Value) -> Option<Error> {
    let err = rpc.get("error")?;
    Some(Error::Rpc {
        code: err.get("code").and_then(Value::as_i64).unwrap_or(0),
        message: err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("error rpc")
            .to_string(),
    })
}

pub fn parse_tools_list(result: &Value) -> Result<Vec<RemoteTool>, Error> {
    let tools = result
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::msg("tools/list sin array tools"))?;
    let mut out = Vec::with_capacity(tools.len());
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::msg("herramienta sin nombre"))?;
        let description = tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let parameters = tool
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
        out.push(RemoteTool {
            name: name.to_string(),
            description,
            parameters,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn parse_call_result(result: &Value) -> Result<Value, Error> {
    if result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let text = content_text(result);
        return Err(Error::msg(if text.is_empty() {
            "la herramienta devolvió un error".into()
        } else {
            text
        }));
    }
    if let Some(structured) = result.get("structuredContent")
        && !structured.is_null()
    {
        return Ok(structured.clone());
    }
    let text = content_text(result);
    if text.is_empty() {
        return Ok(result.clone());
    }
    Ok(serde_json::from_str(&text).unwrap_or_else(|_| json!(text)))
}

fn content_text(result: &Value) -> String {
    result
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            if item.get("type").and_then(Value::as_str).unwrap_or("text") == "text" {
                item.get("text").and_then(Value::as_str).map(str::to_string)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn mcp_endpoint(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.ends_with("/mcp") || base.contains("/mcp/") {
        base.to_string()
    } else {
        format!("{base}/mcp")
    }
}

pub fn json_id(value: &Value) -> Option<i64> {
    value
        .get("id")
        .and_then(|id| id.as_i64().or_else(|| id.as_u64().map(|n| n as i64)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_appends_mcp_once() {
        assert_eq!(mcp_endpoint("http://127.0.0.1:3100"), "http://127.0.0.1:3100/mcp");
        assert_eq!(
            mcp_endpoint("https://example.com/mcp"),
            "https://example.com/mcp"
        );
    }

    #[test]
    fn parses_tools_and_call() {
        let tools = parse_tools_list(&json!({
            "tools": [{ "name": "list_tasks", "description": "List", "inputSchema": { "type": "object" } }]
        }))
        .unwrap();
        assert_eq!(tools[0].name, "list_tasks");
        let value = parse_call_result(&json!({
            "content": [{ "type": "text", "text": "{\"ok\":true}" }]
        }))
        .unwrap();
        assert_eq!(value["ok"], true);
    }
}
