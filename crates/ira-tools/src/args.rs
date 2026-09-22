use crate::error::ToolError;

pub fn require_str<'a>(
    args: &'a serde_json::Value,
    key: &'static str,
) -> Result<&'a str, ToolError> {
    match args.get(key).and_then(|v| v.as_str()).map(str::trim) {
        Some(s) if !s.is_empty() => Ok(s),
        _ => Err(ToolError::MissingArg(key)),
    }
}

pub fn opt_str<'a>(args: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

pub fn require_i64(args: &serde_json::Value, key: &'static str) -> Result<i64, ToolError> {
    opt_i64(args, key).ok_or(ToolError::MissingArg(key))
}

pub fn opt_i64(args: &serde_json::Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_u64().and_then(|n| i64::try_from(n).ok()))
            .or_else(|| v.as_f64().map(|n| n as i64))
            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
    })
}

pub fn opt_f64(args: &serde_json::Value, key: &str) -> Option<f64> {
    args.get(key).and_then(|v| {
        v.as_f64()
            .or_else(|| v.as_i64().map(|n| n as f64))
            .or_else(|| v.as_u64().map(|n| n as f64))
            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
    })
}

pub fn opt_u64(args: &serde_json::Value, key: &str) -> Option<u64> {
    args.get(key).and_then(|v| {
        v.as_u64()
            .or_else(|| v.as_i64().and_then(|n| u64::try_from(n).ok()))
            .or_else(|| v.as_f64().map(|n| n as u64))
            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
    })
}

pub fn opt_bool(args: &serde_json::Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| {
        v.as_bool().or_else(|| match v.as_str().map(str::trim) {
            Some("true") | Some("1") | Some("yes") => Some(true),
            Some("false") | Some("0") | Some("no") => Some(false),
            _ => None,
        })
    })
}

pub fn opt_str_list(args: &serde_json::Value, key: &str) -> Vec<String> {
    match args.get(key) {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        Some(serde_json::Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                Vec::new()
            } else {
                vec![t.to_string()]
            }
        }
        _ => Vec::new(),
    }
}
