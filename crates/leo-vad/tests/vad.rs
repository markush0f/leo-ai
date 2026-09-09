use leo_audio::{samples_per_frame, ML_RATE};
use leo_vad::{Vad, VadConfig, VadEvent};

#[test]
fn silence_stays_silence() {
    let mut vad = Vad::new(VadConfig::default());
    let zeros = vec![0.0f32; samples_per_frame(ML_RATE)];
    let event = vad.push(&zeros).unwrap();
    assert_eq!(event, VadEvent::Silence);
}
