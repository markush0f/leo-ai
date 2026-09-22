use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("pulse: {0}")]
    Pulse(String),
    #[error("dispositivo no disponible: {0}")]
    Device(String),
    #[error("hilo de audio: {0}")]
    Thread(String),
    #[error("el player está cerrado")]
    Closed,
}

impl AudioError {
    pub fn pulse(msg: impl Into<String>) -> Self {
        Self::Pulse(msg.into())
    }
}
