use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

const API_URL: &str = "https://models.dev/api.json";
const CACHE_TTL: Duration = Duration::from_secs(12 * 60 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogModel {
    pub provider: String,
    pub id: String,
    pub name: String,
    pub reasoning: bool,
    pub reasoning_options: Vec<String>,
    pub context_window: Option<u64>,
    pub output_limit: Option<u64>,
    pub release_date: Option<String>,
    pub last_updated: Option<String>,
}

static CACHE: OnceLock<RwLock<Option<(Instant, Arc<Vec<CatalogModel>>)>>> = OnceLock::new();

pub async fn load() -> Result<Arc<Vec<CatalogModel>>, String> {
    let cache = CACHE.get_or_init(|| RwLock::new(None));
    if let Some((loaded_at, models)) = cache.read().await.as_ref()
        && loaded_at.elapsed() < CACHE_TTL
    {
        return Ok(models.clone());
    }

    match fetch().await {
        Ok(models) => {
            let models = Arc::new(models);
            *cache.write().await = Some((Instant::now(), models.clone()));
            let _ = write_disk_cache(&models);
            Ok(models)
        }
        Err(fetch_error) => {
            if let Ok(models) = read_disk_cache() {
                let models = Arc::new(models);
                *cache.write().await = Some((Instant::now(), models.clone()));
                return Ok(models);
            }
            if let Some((_, models)) = cache.read().await.as_ref() {
                return Ok(models.clone());
            }
            Err(format!(
                "Models.dev no disponible y sin catálogo local: {fetch_error}"
            ))
        }
    }
}

pub fn provider_id(kind: &str) -> Option<&'static str> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "gpt" => Some("openai"),
        "codex" => Some("openai"),
        "grok" => Some("xai"),
        "claude" => Some("anthropic"),
        "ollama" => Some("ollama"),
        _ => None,
    }
}

pub fn models_for<'a>(all: &'a [CatalogModel], provider: &str) -> Vec<&'a CatalogModel> {
    all.iter()
        .filter(|model| model.provider == provider)
        .collect()
}

async fn fetch() -> Result<Vec<CatalogModel>, String> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("ira-ai")
        .build()
        .map_err(|error| error.to_string())?;
    let response = http
        .get(API_URL)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Models.dev respondió HTTP {status}"));
    }
    let data = response
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())?;
    parse(&data)
}

fn parse(data: &Value) -> Result<Vec<CatalogModel>, String> {
    let providers = data
        .as_object()
        .ok_or_else(|| "Models.dev devolvió un catálogo inválido".to_string())?;
    let mut models = Vec::new();
    for (provider, entry) in providers {
        let Some(provider_models) = entry.get("models").and_then(Value::as_object) else {
            continue;
        };
        for (key, value) in provider_models {
            let context_window = value.pointer("/limit/context").and_then(Value::as_u64);
            if context_window == Some(0)
                || value
                    .pointer("/modalities/output")
                    .and_then(Value::as_array)
                    .is_some_and(|modalities| {
                        !modalities
                            .iter()
                            .any(|modality| modality.as_str() == Some("text"))
                    })
            {
                continue;
            }
            let id = value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(key)
                .to_string();
            let reasoning = value
                .get("reasoning")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let reasoning_options = value
                .get("reasoning_options")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|option| option.get("type").and_then(Value::as_str) == Some("effort"))
                .filter_map(|option| option.get("values").and_then(Value::as_array))
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            let model_name = value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(&id)
                .to_string();
            let limit = value.get("limit");
            models.push(CatalogModel {
                provider: provider.clone(),
                id,
                name: model_name,
                reasoning,
                reasoning_options,
                context_window,
                output_limit: limit
                    .and_then(|limit| limit.get("output"))
                    .and_then(Value::as_u64),
                release_date: value
                    .get("release_date")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                last_updated: value
                    .get("last_updated")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
    }
    models.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    if models.is_empty() {
        return Err("Models.dev no devolvió modelos".into());
    }
    Ok(models)
}

fn cache_path() -> std::path::PathBuf {
    ira_store::ira_home().join("models-dev.json")
}

fn write_disk_cache(models: &[CatalogModel]) -> Result<(), String> {
    let path = cache_path();
    let parent = path
        .parent()
        .ok_or_else(|| "ruta de caché inválida".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_vec(models).map_err(|error| error.to_string())?;
    std::fs::write(&tmp, body).map_err(|error| error.to_string())?;
    std::fs::rename(tmp, path).map_err(|error| error.to_string())
}

fn read_disk_cache() -> Result<Vec<CatalogModel>, String> {
    let body = std::fs::read(cache_path()).map_err(|error| error.to_string())?;
    serde_json::from_slice(&body).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reasoning_power_and_provider_specific_ids() {
        let value = serde_json::json!({
            "openai": {
                "models": {
                    "gpt-6-luna": {
                        "id": "gpt-6-luna",
                        "name": "GPT-6 Luna",
                        "reasoning": true,
                        "reasoning_options": [{"type": "effort", "values": ["low", "high", "xhigh"]}],
                        "limit": {"context": 1050000, "output": 128000}
                    }
                }
            }
        });
        let parsed = parse(&value).unwrap();
        assert_eq!(parsed[0].provider, "openai");
        assert_eq!(parsed[0].name, "GPT-6 Luna");
        assert_eq!(parsed[0].reasoning_options, ["low", "high", "xhigh"]);
        assert_eq!(parsed[0].context_window, Some(1_050_000));
        assert_eq!(provider_id("codex"), Some("openai"));
    }
}
