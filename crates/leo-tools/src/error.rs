use std::fmt::Display;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("{0}")]
    Message(String),
    #[error("falta el argumento `{0}`")]
    MissingArg(&'static str),
    #[error("herramienta desconocida: {0}")]
    Unknown(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl ToolError {
    pub fn msg(m: impl Into<String>) -> Self {
        Self::Message(m.into())
    }

    pub fn from_display(err: impl Display) -> Self {
        Self::Message(err.to_string())
    }

    pub fn to_json(&self) -> String {
        serde_json::json!({ "ok": false, "error": self.to_string() }).to_string()
    }
}

pub fn stringify<E: Display>(result: Result<serde_json::Value, E>) -> Result<String, ToolError> {
    result
        .map(|value| value.to_string())
        .map_err(ToolError::from_display)
}
