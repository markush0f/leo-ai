use std::path::PathBuf;

use serde::Deserialize;

use leo_store::database_url;
use leo_telegram::parse_allow_users;

pub struct BotConfig {
    pub token: String,
    pub allow_users: Vec<i64>,
    pub database_url: String,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    #[serde(default)]
    telegram: TelegramSection,
}

#[derive(Debug, Default, Deserialize)]
struct TelegramSection {
    token: Option<String>,
    #[serde(default)]
    allow_users: Vec<i64>,
}

impl BotConfig {
    pub fn load(
        token_flag: Option<String>,
        database_url_flag: Option<String>,
    ) -> Result<Self, String> {
        let file = load_file();
        let token = nonempty(token_flag)
            .or_else(|| nonempty(std::env::var("TELEGRAM_BOT_TOKEN").ok()))
            .or_else(|| file.telegram.token.clone().and_then(|s| nonempty(Some(s))))
            .unwrap_or_default();

        let allow_users = std::env::var("TELEGRAM_ALLOW_USERS")
            .ok()
            .map(|s| parse_allow_users(&s))
            .filter(|v| !v.is_empty())
            .unwrap_or(file.telegram.allow_users);

        let database_url = database_url_flag.unwrap_or_else(database_url);
        Ok(Self {
            token,
            allow_users,
            database_url,
        })
    }
}

fn load_file() -> FileConfig {
    let path = config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return FileConfig::default();
    };
    match toml::from_str(&text) {
        Ok(cfg) => cfg,
        Err(err) => {
            tracing::warn!(%err, path = %path.display(), "config inválida");
            FileConfig::default()
        }
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".").join(".config"))
        .join("leo-ai/config.toml")
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    })
}
