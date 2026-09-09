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

pub trait WakeSpotter: Send {
    fn push(&mut self, samples: &[f32]) -> Option<WakeHit>;
}

pub struct NoopWake;

impl WakeSpotter for NoopWake {
    fn push(&mut self, _samples: &[f32]) -> Option<WakeHit> {
        None
    }
}

/// rustpotter 3.0.2 arrastra candle-core 0.2, que no compila en Rust 1.98.
/// El trait queda listo; mientras tanto la activación es `leo-ctl listen`.
pub fn load_wake(model_path: Option<&Path>) -> Result<Box<dyn WakeSpotter>, WakeError> {
    if let Some(path) = model_path {
        if path.exists() {
            tracing::warn!(
                path = %path.display(),
                "modelo de wake word presente, pero el detector aún no está cableado"
            );
        }
    } else {
        tracing::info!("wake word: usa `leo-ctl listen` (o un atajo de teclado a ese comando)");
    }
    Ok(Box::new(NoopWake))
}
