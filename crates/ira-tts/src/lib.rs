//! Audio synthesis decoupled from playback.
//!
//! The available implementation, [`NullTts`], emits a confirmation tone;
//! it does not turn text into intelligible speech.

use ira_audio::{ML_RATE, sine_beep};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TtsError {
    #[error("tts: {0}")]
    Failed(String),
}

/// Generated mono audio and its sample rate in hertz, ready for the player.
pub struct Pcm {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Synthesis extension point for backends such as Piper or Kokoro; playback lives in `ira-audio`.
pub trait TtsEngine: Send {
    /// Synchronously generates the complete audio without playing it.
    fn synthesize(&self, text: &str) -> Result<Pcm, TtsError>;
}

/// Model-free fallback: a beep whose duration grows with the text's byte length.
pub struct NullTts;

impl TtsEngine for NullTts {
    fn synthesize(&self, text: &str) -> Result<Pcm, TtsError> {
        let ms = (180 + text.len() as u32 * 12).min(1200);
        Ok(Pcm {
            samples: sine_beep(440.0, ms, ML_RATE, 0.16),
            sample_rate: ML_RATE,
        })
    }
}
