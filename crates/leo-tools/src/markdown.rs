/// Convierte markdown sencillo a bloques AppFlowy (delta).
pub fn to_blocks(markdown: &str) -> Vec<serde_json::Value> {
    let mut blocks = Vec::new();
    let mut lines = markdown.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "---" || trimmed == "***" {
            blocks.push(serde_json::json!({"type": "divider", "data": {}}));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("```") {
            let language = rest.trim();
            let mut code = String::new();
            while let Some(next) = lines.next() {
                if next.trim_start().starts_with("```") {
                    break;
                }
                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(next);
            }
            let mut data = serde_json::json!({"delta": delta(&code)});
            if !language.is_empty() {
                data["language"] = language.into();
            }
            blocks.push(serde_json::json!({"type": "code", "data": data}));
            continue;
        }
        if let Some((level, text)) = heading(trimmed) {
            blocks.push(serde_json::json!({
                "type": "heading",
                "data": {"level": level, "delta": delta(text)}
            }));
            continue;
        }
        if let Some(text) = trimmed.strip_prefix("> ") {
            blocks.push(serde_json::json!({
                "type": "quote",
                "data": {"delta": delta(text)}
            }));
            continue;
        }
        if let Some((checked, text)) = todo_item(trimmed) {
            blocks.push(serde_json::json!({
                "type": "todo_list",
                "data": {"checked": checked, "delta": delta(text)}
            }));
            continue;
        }
        if let Some(text) = bullet(trimmed) {
            blocks.push(serde_json::json!({
                "type": "bulleted_list",
                "data": {"delta": delta(text)}
            }));
            continue;
        }
        if let Some(text) = numbered(trimmed) {
            blocks.push(serde_json::json!({
                "type": "numbered_list",
                "data": {"delta": delta(text)}
            }));
            continue;
        }
        blocks.push(serde_json::json!({
            "type": "paragraph",
            "data": {"delta": delta(trimmed)}
        }));
    }
    if blocks.is_empty() {
        blocks.push(serde_json::json!({
            "type": "paragraph",
            "data": {"delta": delta("")}
        }));
    }
    blocks
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let bytes = line.as_bytes();
    if bytes.first() != Some(&b'#') {
        return None;
    }
    let mut n = 0usize;
    while n < bytes.len() && bytes[n] == b'#' {
        n += 1;
    }
    if n == 0 || n > 6 || bytes.get(n) != Some(&b' ') {
        return None;
    }
    Some((n.min(3) as u8, line[n + 1..].trim()))
}

fn todo_item(line: &str) -> Option<(bool, &str)> {
    for prefix in ["- [ ] ", "* [ ] ", "- [x] ", "* [x] ", "- [X] ", "* [X] "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some((prefix.contains('x') || prefix.contains('X'), rest));
        }
    }
    None
}

fn bullet(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
}

fn numbered(line: &str) -> Option<&str> {
    let (idx, rest) = line.split_once(". ")?;
    if idx.chars().all(|c| c.is_ascii_digit()) && !idx.is_empty() {
        Some(rest)
    } else {
        None
    }
}

fn delta(text: &str) -> Vec<serde_json::Value> {
    vec![serde_json::json!({"insert": text})]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_lists_and_code() {
        let blocks = to_blocks(
            "# Título\n\n- uno\n1. dos\n- [ ] tarea\n\n```rust\nfn x() {}\n```\n\n> cita\n---\npárrafo",
        );
        let types: Vec<_> = blocks
            .iter()
            .map(|b| b["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            types,
            [
                "heading",
                "bulleted_list",
                "numbered_list",
                "todo_list",
                "code",
                "quote",
                "divider",
                "paragraph"
            ]
        );
        assert_eq!(blocks[0]["data"]["level"], 1);
        assert_eq!(blocks[3]["data"]["checked"], false);
        assert_eq!(blocks[4]["data"]["language"], "rust");
    }

    #[test]
    fn empty_becomes_paragraph() {
        let blocks = to_blocks("   \n");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "paragraph");
    }
}
