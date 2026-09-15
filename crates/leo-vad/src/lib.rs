//! Stateful 16 kHz WebRTC voice detection with minimum speech and silence hangover.
//!
//! Thresholds count frames, not samples. The voice pipeline supplies 20 ms
//! frames, making the default 25-frame hangover equivalent to 500 ms.

use leo_audio::{ML_RATE, f32_to_i16, samples_per_frame};
use thiserror::Error;
use webrtc_vad::{SampleRate, Vad as WebrtcVad, VadMode};

#[derive(Debug, Error)]
pub enum VadError {
    #[error("frame de VAD inválido (se esperan 10/20/30 ms)")]
    BadFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Frame classification; `SpeechEnded` marks segment closure after the hangover.
pub enum VadEvent {
    Silence,
    Speech,
    SpeechEnded,
}

pub struct VadConfig {
    /// Silent frames required to close a speech segment.
    pub hangover_frames: u32,
    /// Minimum voiced frames required for a valid segment.
    pub min_speech_frames: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            hangover_frames: 25, // 500 ms at 20 ms/frame
            min_speech_frames: 8,
        }
    }
}

/// WebRTC VAD with hangover. `webrtc_vad::Vad` is not `Send`; it stays on the audio thread.
pub struct Vad {
    inner: WebrtcVad,
    cfg: VadConfig,
    in_speech: bool,
    speech_frames: u32,
    silence_frames: u32,
}

impl Vad {
    pub fn new(cfg: VadConfig) -> Self {
        let inner = WebrtcVad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Quality);
        Self {
            inner,
            cfg,
            in_speech: false,
            speech_frames: 0,
            silence_frames: 0,
        }
    }

    pub fn reset(&mut self) {
        self.inner.reset();
        self.inner.set_sample_rate(SampleRate::Rate16kHz);
        self.inner.set_mode(VadMode::Quality);
        self.in_speech = false;
        self.speech_frames = 0;
        self.silence_frames = 0;
    }

    pub fn push(&mut self, samples: &[f32]) -> Result<VadEvent, VadError> {
        let expected = samples_per_frame(ML_RATE);
        let frame = if samples.len() == expected {
            samples
        } else if samples.len() > expected {
            &samples[..expected]
        } else {
            return Ok(if self.in_speech {
                VadEvent::Speech
            } else {
                VadEvent::Silence
            });
        };

        let i16s = f32_to_i16(frame);
        let voiced = self
            .inner
            .is_voice_segment(&i16s)
            .map_err(|_| VadError::BadFrame)?;

        if voiced {
            self.speech_frames = self.speech_frames.saturating_add(1);
            self.silence_frames = 0;
            if !self.in_speech && self.speech_frames >= self.cfg.min_speech_frames {
                self.in_speech = true;
            }
            return Ok(if self.in_speech {
                VadEvent::Speech
            } else {
                VadEvent::Silence
            });
        }

        self.speech_frames = 0;
        if self.in_speech {
            self.silence_frames = self.silence_frames.saturating_add(1);
            if self.silence_frames >= self.cfg.hangover_frames {
                self.in_speech = false;
                self.silence_frames = 0;
                return Ok(VadEvent::SpeechEnded);
            }
            return Ok(VadEvent::Speech);
        }
        Ok(VadEvent::Silence)
    }
}
