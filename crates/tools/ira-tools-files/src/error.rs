#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn msg(m: impl Into<String>) -> Self {
        Self::Message(m.into())
    }
}
