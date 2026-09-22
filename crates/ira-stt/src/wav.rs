//! In-memory WAV encoding without audio-device dependencies.

/// Builds a little-endian, mono PCM16 RIFF WAV with a 44-byte header.
///
/// Clamps samples to `[-1, 1]` before quantization. `sample_rate` is in hertz
/// and only describes the audio: this function does not resample. Callers must
/// supply a valid rate and audio that fits RIFF's 32-bit size fields.
pub fn pcm_f32_to_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let mut pcm16 = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        pcm16.extend_from_slice(&v.to_le_bytes());
    }
    let data_len = pcm16.len() as u32;
    let mut wav = Vec::with_capacity(44 + pcm16.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(&pcm16);
    wav
}
