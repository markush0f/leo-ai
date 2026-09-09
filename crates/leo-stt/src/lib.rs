mod grok;
mod wav;

use thiserror::Error;

pub use grok::GrokStt;
pub use wav::pcm_f32_to_wav;

#[derive(Debug, Error)]
pub enum SttError {
    #[error("stt: {0}")]
    Failed(String),
}

pub trait SttEngine: Send {
    fn transcribe(&self, pcm: &[f32], sample_rate: u32) -> Result<Option<String>, SttError>;
}

/// Sin clave/API: no inventa texto para no mandar basura al LLM.
pub struct NullStt;

impl SttEngine for NullStt {
    fn transcribe(&self, pcm: &[f32], _sample_rate: u32) -> Result<Option<String>, SttError> {
        if !pcm.is_empty() {
            tracing::warn!("stt no configurado; la voz no llega al modelo");
        }
        Ok(None)
    }
}
