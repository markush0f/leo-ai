use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use leo_audio::{AudioFrame, Capture, ML_RATE, Player, play_beep};
use leo_stt::SttEngine;
use leo_tts::TtsEngine;
use leo_vad::{Vad, VadConfig};
use leo_wake::WakeSpotter;

use crate::llm::LlmEngine;
use crate::session::{Action, Command, Session, SessionConfig, SessionEvent, State};

pub struct EngineConfig {
    pub source: String,
    pub sink: String,
    pub session: SessionConfig,
    pub vad: VadConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            source: "@DEFAULT_SOURCE@".into(),
            sink: "@DEFAULT_SINK@".into(),
            session: SessionConfig::default(),
            vad: VadConfig::default(),
        }
    }
}

/// Control and observation channels for the voice thread.
///
/// `state` is the latest published state. Sending a command does not mean the
/// thread has processed it yet, especially while a provider call is in progress.
pub struct EngineHandle {
    pub cmds: Sender<Command>,
    pub events: Receiver<SessionEvent>,
    pub state: std::sync::Arc<std::sync::Mutex<State>>,
}

/// Spawns the thread that owns the providers and processes the voice session.
///
/// Success confirms thread creation, not device initialization. Subsequent
/// failures are reported through `tracing`.
pub fn spawn_engine(
    cfg: EngineConfig,
    wake: Box<dyn WakeSpotter>,
    stt: Box<dyn SttEngine>,
    llm: Box<dyn LlmEngine>,
    tts: Box<dyn TtsEngine>,
) -> Result<EngineHandle, String> {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (ev_tx, ev_rx) = mpsc::channel();
    let state = std::sync::Arc::new(std::sync::Mutex::new(State::Idle));
    let state_t = std::sync::Arc::clone(&state);

    std::thread::Builder::new()
        .name("leo-engine".into())
        .spawn(move || {
            if let Err(err) = run_engine(cfg, cmd_rx, ev_tx, state_t, wake, stt, llm, tts) {
                tracing::error!(%err, "engine");
            }
        })
        .map_err(|e| e.to_string())?;

    Ok(EngineHandle {
        cmds: cmd_tx,
        events: ev_rx,
        state,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_engine(
    cfg: EngineConfig,
    cmds: Receiver<Command>,
    events: Sender<SessionEvent>,
    state: std::sync::Arc<std::sync::Mutex<State>>,
    mut wake: Box<dyn WakeSpotter>,
    stt: Box<dyn SttEngine>,
    llm: Box<dyn LlmEngine>,
    tts: Box<dyn TtsEngine>,
) -> Result<(), String> {
    let capture = Capture::start(&cfg.source).map_err(|e| e.to_string())?;
    let player = Player::start(&cfg.sink).map_err(|e| e.to_string())?;
    let mut vad = Vad::new(cfg.vad);
    let mut session = Session::new(cfg.session);
    let mut was_playing = false;

    loop {
        while let Ok(cmd) = cmds.try_recv() {
            if matches!(cmd, Command::Stop) && session.state == State::Idle {
                // Stop cancels the session; it never shuts down the daemon.
            }
            let actions = session.on_command(cmd);
            apply_actions(
                &mut session,
                &player,
                stt.as_ref(),
                llm.as_ref(),
                tts.as_ref(),
                actions,
                &events,
                &state,
            );
            publish_state(&state, session.state, &events);
        }

        match capture.recv_timeout(Duration::from_millis(20)) {
            Ok(frame) => {
                handle_frame(
                    &mut session,
                    &mut vad,
                    wake.as_mut(),
                    &player,
                    stt.as_ref(),
                    llm.as_ref(),
                    tts.as_ref(),
                    frame,
                    &events,
                    &state,
                );
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let playing = player.is_playing();
        if was_playing && !playing {
            for ev in session.on_playback_finished() {
                let _ = events.send(ev);
            }
            publish_state(&state, session.state, &events);
        }
        was_playing = playing;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_frame(
    session: &mut Session,
    vad: &mut Vad,
    wake: &mut dyn WakeSpotter,
    player: &Player,
    stt: &dyn SttEngine,
    llm: &dyn LlmEngine,
    tts: &dyn TtsEngine,
    frame: AudioFrame,
    events: &Sender<SessionEvent>,
    state: &std::sync::Arc<std::sync::Mutex<State>>,
) {
    let decision = match vad.push(&frame.samples) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(%err, "vad");
            return;
        }
    };

    if session.state == State::Idle {
        if let Some(hit) = wake.push(&frame.samples) {
            let (actions, evs) = session.on_wake(&hit);
            for ev in evs {
                let _ = events.send(ev);
            }
            apply_actions(session, player, stt, llm, tts, actions, events, state);
            let _ = play_beep(player, 880.0, 120);
        }
        return;
    }

    let (actions, evs) = session.on_frame(&frame, decision);
    for ev in evs {
        let _ = events.send(ev);
    }
    apply_actions(session, player, stt, llm, tts, actions, events, state);
}

#[allow(clippy::too_many_arguments)]
fn apply_actions(
    session: &mut Session,
    player: &Player,
    stt: &dyn SttEngine,
    llm: &dyn LlmEngine,
    tts: &dyn TtsEngine,
    actions: Vec<Action>,
    events: &Sender<SessionEvent>,
    state: &std::sync::Arc<std::sync::Mutex<State>>,
) {
    for action in actions {
        match action {
            Action::StopPlayback => {
                let _ = player.stop();
            }
            Action::Play { pcm, sample_rate } => {
                let _ = player.play(pcm, sample_rate);
            }
            Action::Transcribe(pcm) => {
                let text = match stt.transcribe(&pcm, ML_RATE) {
                    Ok(t) => t,
                    Err(err) => {
                        tracing::error!(%err, "stt");
                        None
                    }
                };
                let (next, evs) = session.on_transcript(text);
                for ev in evs {
                    let _ = events.send(ev);
                }
                apply_actions(session, player, stt, llm, tts, next, events, state);
            }
            Action::AskLlm(text) => {
                tracing::info!(user = %text, "llm");
                let reply = match llm.reply(&text) {
                    Ok(r) => r,
                    Err(err) => {
                        tracing::error!(%err, "llm");
                        format!("no pude consultar el modelo: {err}")
                    }
                };
                let (next, evs) = session.on_llm_reply(reply);
                for ev in evs {
                    let _ = events.send(ev);
                }
                apply_actions(session, player, stt, llm, tts, next, events, state);
            }
            Action::Synthesize(text) => match tts.synthesize(&text) {
                Ok(pcm) => {
                    let _ = player.play(pcm.samples, pcm.sample_rate);
                }
                Err(err) => tracing::error!(%err, "tts"),
            },
        }
    }
    publish_state(state, session.state, events);
}

fn publish_state(
    slot: &std::sync::Arc<std::sync::Mutex<State>>,
    current: State,
    events: &Sender<SessionEvent>,
) {
    if let Ok(mut guard) = slot.lock() {
        if *guard != current {
            *guard = current;
            let _ = events.send(SessionEvent::State(current));
        } else {
            *guard = current;
        }
    }
}
