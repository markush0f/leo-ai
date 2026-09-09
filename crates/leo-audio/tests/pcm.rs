use leo_audio::{resample_mono, rms, DEVICE_RATE, ML_RATE};

#[test]
fn downsample_48k_is_exact_third() {
    let input: Vec<f32> = (0..960).map(|i| i as f32).collect();
    let out = resample_mono(&input, DEVICE_RATE, ML_RATE);
    assert_eq!(out.len(), 320);
    assert!((out[0] - 1.0).abs() < f32::EPSILON);
}

#[test]
fn rms_of_zeros_is_zero() {
    assert_eq!(rms(&[0.0, 0.0, 0.0]), 0.0);
}
