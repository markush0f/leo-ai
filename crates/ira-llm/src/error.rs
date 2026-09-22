use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("falta la variable de entorno {0}")]
    MissingKey(&'static str),
    #[error("faltan credenciales OAuth de {0}")]
    MissingCredentials(&'static str),
    #[error("credenciales inválidas: {0}")]
    InvalidCredentials(String),
    #[error("autenticación: {0}")]
    Authentication(String),
    #[error("almacenamiento de credenciales: {0}")]
    TokenStore(String),
    #[error("proveedor desconocido: {0}")]
    UnknownProvider(String),
    #[error("http {status}: {body}")]
    Http { status: u16, body: String },
    #[error("respuesta vacía de {0}")]
    Empty(&'static str),
    #[error("red: {0}")]
    Network(#[from] reqwest::Error),
    #[error("url: {0}")]
    Url(#[from] url::ParseError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("demasiadas vueltas de tools")]
    ToolLoop,
}
