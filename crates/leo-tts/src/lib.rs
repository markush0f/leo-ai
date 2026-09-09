use leo_audio::{sine_beep, ML_RATE};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TtsError {
    #[error("tts: {0}")]
    Failed(String),
}

pub struct Pcm {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Síntesis local. Piper/Kokoro se enchufan aquí; el player vive en `leo-audio`.
pub trait TtsEngine: Send {
    fn synthesize(&self, text: &str) -> Result<Pcm, TtsError>;
}

/// Sin modelo: un beep cuya duración escala un poco con el texto.
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
