#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("callmebot http {status}: {body}")]
    Http { status: u16, body: String },
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

pub(crate) fn redact(body: &str, secret: &str) -> String {
    if secret.is_empty() {
        return body.to_string();
    }
    body.replace(secret, "[redacted]")
}
