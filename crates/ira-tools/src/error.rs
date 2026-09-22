use std::fmt::Display;

/// Failure produced while validating or executing a tool call.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// Free-form tool or integration error.
    #[error("{0}")]
    Message(String),
    /// Required argument was absent or invalid.
    #[error("falta el argumento `{0}`")]
    MissingArg(&'static str),
    /// Requested tool is not registered.
    #[error("herramienta desconocida: {0}")]
    Unknown(String),
    /// Filesystem or process I/O failed.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// JSON serialization or deserialization failed.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl ToolError {
    /// Creates a free-form tool error.
    pub fn msg(m: impl Into<String>) -> Self {
        Self::Message(m.into())
    }

    /// Converts any displayable error into a free-form tool error.
    pub fn from_display(err: impl Display) -> Self {
        Self::Message(err.to_string())
    }

    /// Serializes this error as the JSON payload returned to the model.
    pub fn to_json(&self) -> String {
        serde_json::json!({ "ok": false, "error": self.to_string() }).to_string()
    }
}

/// Serializes a JSON value or maps its source error into [`ToolError`].
pub fn stringify<E: Display>(result: Result<serde_json::Value, E>) -> Result<String, ToolError> {
    result
        .map(|value| value.to_string())
        .map_err(ToolError::from_display)
}
