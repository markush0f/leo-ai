use leo_audio::{AudioFrame, ML_RATE};
use leo_core::{Action, Command, Session, SessionConfig, SessionEvent, State};
use leo_vad::VadEvent;

fn frame(amp: f32) -> AudioFrame {
    AudioFrame {
        samples: vec![amp; 320],
        sample_rate: ML_RATE,
    }
}

#[test]
fn listen_speech_endpoint_transcribe() {
    let mut s = Session::new(SessionConfig::default());
    s.on_command(Command::Listen);
    assert_eq!(s.state, State::Listening);
    let (_a, ev) = s.on_frame(&frame(0.2), VadEvent::Speech);
    assert_eq!(s.state, State::Recording);
    assert!(ev
        .iter()
        .any(|e| matches!(e, SessionEvent::State(State::Recording))));
    let (actions, _) = s.on_frame(&frame(0.0), VadEvent::SpeechEnded);
    assert!(matches!(actions[0], Action::Transcribe(_)));
    assert_eq!(s.state, State::Transcribing);
}

#[test]
fn barge_in_cuts_playback() {
    let mut s = Session::new(SessionConfig::default());
    s.on_command(Command::Speak("hola".into()));
    assert_eq!(s.state, State::Speaking);
    let (actions, events) = s.on_frame(&frame(0.4), VadEvent::Speech);
    assert!(matches!(actions[0], Action::StopPlayback));
    assert!(events.iter().any(|e| matches!(e, SessionEvent::BargeIn)));
    assert_eq!(s.state, State::Recording);
}

#[test]
fn idle_ignores_audio() {
    let mut s = Session::new(SessionConfig::default());
    let (a, e) = s.on_frame(&frame(0.9), VadEvent::Speech);
    assert!(a.is_empty() && e.is_empty());
}

#[test]
fn transcript_goes_to_llm_then_speech() {
    let mut s = Session::new(SessionConfig::default());
    let (actions, events) = s.on_transcript(Some("qué hora es".into()));
    assert_eq!(s.state, State::Thinking);
    assert!(matches!(actions[0], Action::AskLlm(ref t) if t == "qué hora es"));
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::Transcript(t) if t == "qué hora es")));

    let (actions, events) = s.on_llm_reply("las tres".into());
    assert_eq!(s.state, State::Speaking);
    assert!(matches!(actions[0], Action::Synthesize(ref t) if t == "las tres"));
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::Reply(t) if t == "las tres")));
}

#[test]
fn empty_transcript_returns_to_idle() {
    let mut s = Session::new(SessionConfig::default());
    s.on_command(Command::Listen);
    let (actions, _) = s.on_transcript(Some("   ".into()));
    assert!(actions.is_empty());
    assert_eq!(s.state, State::Idle);
}
