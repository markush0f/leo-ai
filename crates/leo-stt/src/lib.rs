//! Mono audio transcription through a synchronous interface.
//!
//! [`GrokStt`] uploads PCM16 WAV to xAI; [`NullStt`] represents an unconfigured
//! transcriber. `Option` and `Result` distinguish missing text from failures.

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

/// Transcriber transferable to the voice thread; calls may block until completion.
pub trait SttEngine: Send {
    /// Transcribes mono PCM normalized to `[-1, 1]`, with sample rate in hertz.
    ///
    /// Returns `Ok(None)` when no usable text is available. Transport and provider
    /// failures return errors rather than fabricated transcripts.
    fn transcribe(&self, pcm: &[f32], sample_rate: u32) -> Result<Option<String>, SttError>;
}

/// Disabled transcriber: returns no text rather than sending fabricated input to the LLM.
pub struct NullStt;

impl SttEngine for NullStt {
    fn transcribe(&self, pcm: &[f32], _sample_rate: u32) -> Result<Option<String>, SttError> {
        if !pcm.is_empty() {
            tracing::warn!("stt no configurado; la voz no llega al modelo");
        }
        Ok(None)
    }
}
