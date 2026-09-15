//! Mono capture and playback through PulseAudio or PipeWire's Pulse compatibility layer.
//!
//! Devices run at 48 kHz; VAD and STT receive 16 kHz audio in 20 ms frames.
//! PCM utilities are also available without opening an audio device.

mod capture;
mod error;
mod pcm;
mod player;
mod pulse;

pub use capture::{AudioFrame, Capture};
pub use error::AudioError;
pub use pcm::{
    DEVICE_RATE, FRAME_MS, ML_RATE, f32_to_i16, i16_to_f32, resample_mono, rms, samples_per_frame,
    sine_beep,
};
pub use player::{Player, play_beep};
