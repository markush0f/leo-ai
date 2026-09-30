use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item, Table, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssistantPreferences {
    pub active_provider: Option<String>,
    pub active_provider_id: Option<String>,
    pub active_model: Option<String>,
    pub active_effort: Option<String>,
    pub system_prompt: Option<String>,
    pub web_search_enabled: bool,
    pub web_search_context_size: String,
}

impl Default for AssistantPreferences {
    fn default() -> Self {
        Self {
            active_provider: None,
            active_provider_id: None,
            active_model: None,
            active_effort: None,
            system_prompt: None,
            web_search_enabled: true,
            web_search_context_size: "high".into(),
        }
    }
}

pub fn load() -> Result<AssistantPreferences, String> {
    let path = path();
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AssistantPreferences::default());
        }
        Err(error) => return Err(format!("no se pudo leer {}: {error}", path.display())),
    };
    let doc = source
        .parse::<DocumentMut>()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let assistant = doc.get("assistant");
    Ok(AssistantPreferences {
        active_provider: assistant
            .and_then(|table| table.get("active_provider"))
            .and_then(Item::as_str)
            .map(str::to_string),
        active_provider_id: assistant
            .and_then(|table| table.get("active_provider_id"))
            .and_then(Item::as_str)
            .map(str::to_string),
        active_model: assistant
            .and_then(|table| table.get("active_model"))
            .and_then(Item::as_str)
            .map(str::to_string),
        active_effort: assistant
            .and_then(|table| table.get("active_effort"))
            .and_then(Item::as_str)
            .map(str::to_string),
        system_prompt: assistant
            .and_then(|table| table.get("system_prompt"))
            .and_then(Item::as_str)
            .map(str::to_string),
        web_search_enabled: assistant
            .and_then(|table| table.get("web_search_enabled"))
            .and_then(Item::as_bool)
            .unwrap_or(true),
        web_search_context_size: assistant
            .and_then(|table| table.get("web_search_context_size"))
            .and_then(Item::as_str)
            .filter(|value| matches!(*value, "low" | "medium" | "high"))
            .unwrap_or("high")
            .to_string(),
    })
}

pub fn save(preferences: &AssistantPreferences) -> Result<(), String> {
    if !matches!(
        preferences.web_search_context_size.as_str(),
        "low" | "medium" | "high"
    ) {
        return Err("contexto de búsqueda inválido".into());
    }
    let path = path();
    let mut doc = match std::fs::read_to_string(&path) {
        Ok(source) => source
            .parse::<DocumentMut>()
            .map_err(|error| format!("{}: {error}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DocumentMut::new(),
        Err(error) => return Err(format!("no se pudo leer {}: {error}", path.display())),
    };
    update_document(&mut doc, preferences)?;

    let parent = path
        .parent()
        .ok_or_else(|| "ruta de config inválida".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, doc.to_string()).map_err(|error| error.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|error| error.to_string())
}

fn update_document(
    doc: &mut DocumentMut,
    preferences: &AssistantPreferences,
) -> Result<(), String> {
    if !doc.contains_table("assistant") {
        doc["assistant"] = Item::Table(Table::new());
    }
    let assistant = doc["assistant"]
        .as_table_mut()
        .ok_or_else(|| "[assistant] debe ser una tabla en config.toml".to_string())?;
    set_optional(
        assistant,
        "active_provider",
        preferences.active_provider.as_deref(),
    );
    set_optional(
        assistant,
        "active_provider_id",
        preferences.active_provider_id.as_deref(),
    );
    set_optional(
        assistant,
        "active_model",
        preferences.active_model.as_deref(),
    );
    set_optional(
        assistant,
        "active_effort",
        preferences.active_effort.as_deref(),
    );
    set_optional(
        assistant,
        "system_prompt",
        preferences.system_prompt.as_deref(),
    );
    assistant["web_search_enabled"] = Value::from(preferences.web_search_enabled).into();
    assistant["web_search_context_size"] =
        Value::from(preferences.web_search_context_size.as_str()).into();
    Ok(())
}

pub fn save_web_search(enabled: bool, context_size: &str) -> Result<AssistantPreferences, String> {
    let mut preferences = load()?;
    preferences.web_search_enabled = enabled;
    preferences.web_search_context_size = context_size.to_string();
    save(&preferences)?;
    Ok(preferences)
}

fn set_optional(table: &mut Table, key: &str, value: Option<&str>) {
    match value.filter(|value| !value.trim().is_empty()) {
        Some(value) => table[key] = Value::from(value).into(),
        None => {
            table.remove(key);
        }
    }
}

fn path() -> PathBuf {
    ira_store::ira_home().join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_preferences_without_dropping_other_sections() {
        let mut doc = "[audio]\nsink = 'speaker'\n\n[mcp.server]\nurl = 'https://example.test'\n"
            .parse::<DocumentMut>()
            .unwrap();
        let preferences = AssistantPreferences {
            active_provider: Some("codex".into()),
            active_provider_id: Some("provider-id".into()),
            active_model: Some("gpt-6-luna".into()),
            active_effort: Some("high".into()),
            system_prompt: Some("Responde en español".into()),
            web_search_enabled: false,
            web_search_context_size: "medium".into(),
        };

        update_document(&mut doc, &preferences).unwrap();
        let parsed = doc.to_string().parse::<DocumentMut>().unwrap();
        assert_eq!(parsed["audio"]["sink"].as_str(), Some("speaker"));
        assert_eq!(
            parsed["mcp"]["server"]["url"].as_str(),
            Some("https://example.test")
        );
        assert_eq!(
            parsed["assistant"]["active_model"].as_str(),
            Some("gpt-6-luna")
        );
        assert_eq!(
            parsed["assistant"]["active_provider_id"].as_str(),
            Some("provider-id")
        );
        assert_eq!(parsed["assistant"]["active_effort"].as_str(), Some("high"));
        assert_eq!(
            parsed["assistant"]["web_search_enabled"].as_bool(),
            Some(false)
        );
        assert_eq!(
            parsed["assistant"]["web_search_context_size"].as_str(),
            Some("medium")
        );
    }
}
