#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
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
