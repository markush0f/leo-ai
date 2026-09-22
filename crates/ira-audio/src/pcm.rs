//! Mono signal utilities; all sample rates are expressed in hertz.

/// Sample rate used for device capture and playback.
pub const DEVICE_RATE: u32 = 48_000;
/// Input sample rate for voice processing.
pub const ML_RATE: u32 = 16_000;
/// Duration of each audio frame in milliseconds.
pub const FRAME_MS: u32 = 20;

pub fn samples_per_frame(rate: u32) -> usize {
    (rate * FRAME_MS / 1000) as usize
}

/// Root mean square amplitude of a frame; empty input returns zero.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}

/// Quantizes samples to PCM16, clamping amplitudes outside `[-1, 1]`.
pub fn f32_to_i16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect()
}

pub fn i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples
        .iter()
        .map(|s| *s as f32 / i16::MAX as f32)
        .collect()
}

/// Resamples mono PCM; both sample rates must be greater than zero.
///
/// The 48-to-16 kHz path averages triples and drops any incomplete trailing group.
/// Other conversions use linear interpolation, not band-limited antialias
/// filtering. Empty input remains empty.
pub fn resample_mono(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    if from == DEVICE_RATE && to == ML_RATE {
        return input
            .as_chunks::<3>()
            .0
            .iter()
            .map(|c| (c[0] + c[1] + c[2]) / 3.0)
            .collect();
    }
    if from == ML_RATE && to == DEVICE_RATE {
        let mut out = Vec::with_capacity(input.len() * 3);
        for w in input.windows(2) {
            let a = w[0];
            let b = w[1];
            out.push(a);
            out.push(a * (2.0 / 3.0) + b * (1.0 / 3.0));
            out.push(a * (1.0 / 3.0) + b * (2.0 / 3.0));
        }
        if let Some(&last) = input.last() {
            out.push(last);
            out.push(last);
            out.push(last);
        }
        return out;
    }

    let ratio = to as f64 / from as f64;
    let out_len = ((input.len() as f64) * ratio).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);
    let last = input.len() - 1;
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let idx = src.floor() as usize;
        let frac = (src - idx as f64) as f32;
        let a = input[idx.min(last)];
        let b = input[(idx + 1).min(last)];
        out.push(a + (b - a) * frac);
    }
    out
}

pub fn sine_beep(freq_hz: f32, duration_ms: u32, rate: u32, amplitude: f32) -> Vec<f32> {
    let n = (rate as u64 * duration_ms as u64 / 1000) as usize;
    let amp = amplitude.clamp(0.0, 1.0);
    (0..n)
        .map(|i| {
            let t = i as f32 / rate as f32;
            (2.0 * std::f32::consts::PI * freq_hz * t).sin() * amp
        })
        .collect()
}
