use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("falta la variable de entorno {0}")]
    MissingKey(&'static str),
    #[error("proveedor desconocido: {0}")]
    UnknownProvider(String),
    #[error("http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("respuesta vacía de {0}")]
    Empty(&'static str),
    #[error("red: {0}")]
    Network(#[from] reqwest::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("demasiadas vueltas de tools")]
    ToolLoop,
}
