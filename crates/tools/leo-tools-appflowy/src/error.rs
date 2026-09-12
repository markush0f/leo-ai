#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("falta {0} en el entorno")]
    Missing(&'static str),
    #[error("appflowy http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("appflowy: {0}")]
    Msg(String),
    #[error("red: {0}")]
    Network(#[from] reqwest::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
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
