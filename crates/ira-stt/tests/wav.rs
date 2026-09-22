use ira_stt::pcm_f32_to_wav;

#[test]
fn wav_header_is_44_bytes_plus_pcm16() {
    let samples = vec![0.0f32; 160];
    let wav = pcm_f32_to_wav(&samples, 16_000);
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(wav.len(), 44 + 160 * 2);
}
