#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("google http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("red: {0}")]
    Network(#[from] reqwest::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub fn msg(m: impl Into<String>) -> Self {
        Self::Message(m.into())
    }
}

pub(crate) fn clip(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > 400 {
        format!("{}…", t.chars().take(400).collect::<String>())
    } else {
        t.to_string()
    }
}

pub(crate) fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    })
}
