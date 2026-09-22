//! Wake-word detection interface.
//!
//! The current loader returns [`NoopWake`]; providing a model file does not
//! enable detection. An external command must start listening.

use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WakeError {
    #[error("no se pudo cargar el wake word: {0}")]
    Load(String),
}

#[derive(Debug, Clone)]
pub struct WakeHit {
    pub name: String,
    pub score: f32,
}

/// Incremental detector receiving mono frames from the voice pipeline.
pub trait WakeSpotter: Send {
    /// Consumes a frame and returns a hit only when activation is detected.
    fn push(&mut self, samples: &[f32]) -> Option<WakeHit>;
}

pub struct NoopWake;

impl WakeSpotter for NoopWake {
    fn push(&mut self, _samples: &[f32]) -> Option<WakeHit> {
        None
    }
}

/// Returns a no-op detector until a wake-word backend is integrated.
/// Use `ira-ctl listen` to activate the session in the meantime.
pub fn load_wake(model_path: Option<&Path>) -> Result<Box<dyn WakeSpotter>, WakeError> {
    if let Some(path) = model_path {
        if path.exists() {
            tracing::warn!(
                path = %path.display(),
                "modelo de wake word presente, pero el detector aún no está cableado"
            );
        }
    } else {
        tracing::info!("wake word: usa `ira-ctl listen` (o un atajo de teclado a ese comando)");
    }
    Ok(Box::new(NoopWake))
}
