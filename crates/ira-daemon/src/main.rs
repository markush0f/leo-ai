//! Voice daemon binary (`ira-daemon`).
//!
//! Loads catalog, engines, and voice settings from PostgreSQL, constructs
//! blocking STT/LLM/TTS/wake providers, and spawns the voice thread. It then
//! serves one-request/one-response JSON commands on a Unix socket. Chat UIs
//! do not run this pipeline; they send [`ira_ipc::Request`] values (or use
//! `ira-ctl`) while this process is running.
//!
//! # Workspace crates
//!
//! - [`ira_store`] — catalog and the dedicated voice conversation. `connect` /
//!   `migrate` prepare the schema; `apply_secrets_to_env` exports stored keys;
//!   `load` yields [`ira_store::Snapshot`] (active model, STT/TTS/wake
//!   engines, VAD/barge-in, devices, spoken system prompt). `ensure_voice`
//!   plus `append_message` persist transcripts and replies. A leftover
//!   `config.toml` is imported once via [`ira_store::DbOp`] if voice columns
//!   are still at defaults.
//! - [`ira_llm`] — `load_dotenv` and the async [`ira_llm::Client`] built from
//!   the snapshot. Voice turns are single-shot (system + current utterance),
//!   not the chat history used by TUI/Telegram/desktop. Tool execution is not
//!   part of this path.
//! - [`ira_core`] — [`ira_core::Session`] state machine (`idle` → `listening`
//!   → `recording` → `transcribing` → `thinking` → `speaking`) and
//!   [`ira_core::spawn_engine`]. The engine thread owns capture, VAD, STT,
//!   LLM, TTS, and playback. Commands wait while a provider call is in
//!   progress. [`ira_core::NullLlm`] is used when no client can be built.
//! - [`ira_audio`] — used inside `ira-core` (not called from this file).
//!   Pulse/PipeWire capture at 48 kHz, 16 kHz mono frames to VAD/STT;
//!   playback of TTS PCM. An eight-frame capture queue drops when full.
//! - [`ira_vad`] — WebRTC VAD at 16 kHz. Hangover is converted from
//!   `vad_hangover_ms` assuming 20 ms frames.
//! - [`ira_wake`] — [`ira_wake::load_wake`]. The loader currently returns
//!   [`ira_wake::NoopWake`]; a model path does not enable detection. Start
//!   listening with `listen`.
//! - [`ira_stt`] — synchronous [`ira_stt::SttEngine`]. [`ira_stt::GrokStt`]
//!   uploads mono PCM16 WAV via `Handle::block_on` (engine thread only).
//!   [`ira_stt::NullStt`] returns no transcript when Grok or its key is
//!   missing.
//! - [`ira_tts`] — synchronous [`ira_tts::TtsEngine`]. This binary always
//!   injects [`ira_tts::NullTts`], which plays a confirmation tone rather
//!   than spoken text.
//! - [`ira_ipc`] — Unix socket at `$XDG_RUNTIME_DIR/ira-ai.sock` (else
//!   `/tmp/ira-ai.sock`). [`ira_ipc::bind`] removes any existing path first;
//!   only one daemon may own it. Requests: `status`, `listen`, `stop`,
//!   `speak`, `shutdown`. The server does not time out clients.
//!
//! # Local modules
//!
//! - `config`: leftover TOML (`~/.config/ira-ai/config.toml`) and data dirs.
//! - `llm`: `BlockingLlm` bridges the async client to [`ira_core::LlmEngine`]
//!   with `Handle::block_on`.
//!
//! # Startup
//!
//! Database URL follows [`ira_store::database_url`]. Engine spawn succeeding
//! means the thread started, not that Pulse devices opened; later failures
//! go to tracing. Transcripts and replies are appended from the event thread
//! via the Tokio handle.

mod config;
mod llm;

use std::path::PathBuf;

use ira_core::{
    Command, EngineConfig, LlmEngine, NullLlm, SessionConfig, SessionEvent, spawn_engine,
};
use ira_ipc::{Request, Response, bind, read_request, socket_path, write_response};
use ira_store::{
    self as store, DbOp, EngineRole, NewMessage, Snapshot, database_url, ensure_voice,
};
use ira_stt::{GrokStt, NullStt, SttEngine};
use ira_tts::NullTts;
use ira_vad::VadConfig;
use ira_wake::load_wake;
use tokio::runtime::Handle;
use tracing_subscriber::EnvFilter;

use crate::llm::BlockingLlm;

use crate::config::{FileConfig, ensure_dirs};

#[tokio::main]
async fn main() {
    ira_llm::load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    if let Err(err) = run().await {
        tracing::error!(%err, "daemon");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    ensure_dirs();
    let url = database_url();
    let pool = store::connect(&url).await.map_err(|err| {
        format!(
            "no se pudo conectar a postgres ({url}): {err}\narranca la bbdd: docker compose up -d"
        )
    })?;
    store::migrate(&pool).await?;
    let _ = store::apply_secrets_to_env(&pool).await;
    let file = FileConfig::load();
    import_toml_if_needed(&pool, &file).await?;
    let snap = store::load(&pool).await?;
    let engine_cfg = engine_config(&snap);
    tracing::info!(source = %engine_cfg.source, sink = %engine_cfg.sink, "ira-daemon");

    let wake_path = wake_model_path(&snap);
    let wake = load_wake(wake_path.as_deref())?;
    let rt = Handle::current();
    let stt = build_stt(&snap, rt.clone());
    let llm: Box<dyn LlmEngine> = match build_llm(&snap, &pool, rt.clone()) {
        Ok(llm) => llm,
        Err(err) => {
            tracing::warn!(%err, "llm deshabilitado");
            Box::new(NullLlm)
        }
    };
    if let Some(model) = snap.active_model() {
        tracing::info!(model = %model.name, "llm");
    }
    if let Some(stt) = snap.engine(EngineRole::Stt) {
        tracing::info!(kind = %stt.kind, language = %snap.settings.stt_language, "stt");
    }
    if let Some(tts) = snap.engine(EngineRole::Tts) {
        tracing::info!(kind = %tts.kind, "tts");
    }
    let handle = spawn_engine(engine_cfg, wake, stt, llm, Box::new(NullTts))?;
    let voice = ensure_voice(&pool).await?;

    std::thread::Builder::new()
        .name("ira-events".into())
        .spawn({
            let events = handle.events;
            let pool = pool.clone();
            let conv = voice.id;
            let model_id = snap.active_model_id;
            let rt = rt.clone();
            move || {
                while let Ok(ev) = events.recv() {
                    match ev {
                        SessionEvent::State(s) => tracing::info!(state = s.as_str(), "estado"),
                        SessionEvent::Wake { name, score } => {
                            tracing::info!(name, score, "wake")
                        }
                        SessionEvent::Transcript(t) => {
                            tracing::info!(text = %t, "voz");
                            persist_voice(&rt, &pool, conv, NewMessage::user(t));
                        }
                        SessionEvent::Reply(t) => {
                            tracing::info!(text = %t, "ira");
                            persist_voice(&rt, &pool, conv, NewMessage::assistant(t, model_id));
                        }
                        SessionEvent::BargeIn => tracing::info!("barge-in"),
                    }
                }
            }
        })?;

    let sock = socket_path();
    let listener = bind(&sock).await?;
    tracing::info!(path = %sock.display(), "socket");
    tracing::info!("hotkey: asigna un atajo a `ira-ctl listen`");

    let cmds = handle.cmds;
    let state = handle.state;
    let mut shutting_down = false;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("SIGINT");
                break;
            }
            accepted = listener.accept() => {
                let (mut stream, _) = accepted?;
                let req = match read_request(&mut stream).await {
                    Ok(r) => r,
                    Err(err) => {
                        tracing::warn!(%err, "pedido ipc");
                        continue;
                    }
                };
                let current = state
                    .lock()
                    .map(|g| g.as_str().to_string())
                    .unwrap_or_else(|_| "unknown".into());

                let resp = match req {
                    Request::Status => Response::ok(&current, "ok"),
                    Request::Listen => {
                        let _ = cmds.send(Command::Listen);
                        Response::ok("listening", "escuchando")
                    }
                    Request::Stop => {
                        let _ = cmds.send(Command::Stop);
                        Response::ok("idle", "stop")
                    }
                    Request::Speak { text } => {
                        let _ = cmds.send(Command::Speak(text));
                        Response::ok("speaking", "reproduciendo")
                    }
                    Request::Shutdown => {
                        shutting_down = true;
                        let _ = cmds.send(Command::Stop);
                        Response::ok(&current, "apagando")
                    }
                };
                let _ = write_response(&mut stream, &resp).await;
                if shutting_down {
                    break;
                }
            }
        }
    }

    let _ = std::fs::remove_file(&sock);
    Ok(())
}

fn persist_voice(rt: &Handle, pool: &sqlx::PgPool, conversation_id: uuid::Uuid, msg: NewMessage) {
    let pool = pool.clone();
    rt.spawn(async move {
        if let Err(err) = store::append_message(&pool, conversation_id, msg).await {
            tracing::warn!(%err, "voz no guardada");
        }
    });
}

pub(crate) fn engine_config(snap: &Snapshot) -> EngineConfig {
    let hangover_frames = (snap.settings.vad_hangover_ms as u32 / 20).max(1);
    EngineConfig {
        source: snap.settings.audio_source.clone(),
        sink: snap.settings.audio_sink.clone(),
        session: SessionConfig {
            barge_in: snap.settings.barge_in,
            barge_in_rms: snap.settings.barge_in_rms,
            ..SessionConfig::default()
        },
        vad: VadConfig {
            hangover_frames,
            ..VadConfig::default()
        },
    }
}

pub(crate) fn build_llm(
    snap: &Snapshot,
    pool: &sqlx::PgPool,
    rt: Handle,
) -> Result<Box<dyn LlmEngine>, Box<dyn std::error::Error>> {
    let client = store::client_with_pool(snap, pool)?;
    Ok(Box::new(BlockingLlm::new(
        client,
        snap.settings.voice_system_prompt.clone(),
        rt,
    )))
}

fn build_stt(snap: &Snapshot, rt: Handle) -> Box<dyn SttEngine> {
    let kind = snap
        .engine(EngineRole::Stt)
        .map(|e| e.kind.as_str())
        .unwrap_or("null");
    if kind.eq_ignore_ascii_case("grok") {
        if let Some(key) = snap.grok_api_key() {
            let lang = snap.settings.stt_language.clone();
            return Box::new(GrokStt::new(key, rt, Some(lang)));
        }
        tracing::warn!("stt grok sin api key: la voz no se transcribe");
    }
    Box::new(NullStt)
}

fn wake_model_path(snap: &Snapshot) -> Option<PathBuf> {
    let engine = snap.engine(EngineRole::Wake)?;
    engine
        .config
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

async fn import_toml_if_needed(pool: &sqlx::PgPool, file: &FileConfig) -> Result<(), sqlx::Error> {
    let snap = store::load(pool).await?;
    let s = &snap.settings;
    let still_default = s.audio_source == "@DEFAULT_SOURCE@"
        && s.audio_sink == "@DEFAULT_SINK@"
        && s.vad_hangover_ms == 500
        && (s.voice_system_prompt == FileConfig::default().llm.system
            || s.voice_system_prompt
                == "Eres Ira, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta.");
    if !still_default {
        return Ok(());
    }
    store::apply(
        pool,
        DbOp::SetVoiceAudio {
            source: file.audio.source.clone(),
            sink: file.audio.sink.clone(),
        },
    )
    .await?;
    store::apply(
        pool,
        DbOp::SetVad {
            hangover_ms: file.vad.hangover_ms,
            barge_in: file.barge_in.enabled,
            barge_in_rms: file.barge_in.rms_threshold,
        },
    )
    .await?;
    store::apply(pool, DbOp::SetSttLanguage(file.stt.language.clone())).await?;
    store::apply(pool, DbOp::SetVoiceSystem(file.llm.system.clone())).await?;
    tracing::info!("config.toml importado a postgres");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ira_store::stub_snapshot;

    #[test]
    fn llm_uses_catalog_model() {
        let snap = stub_snapshot("grok", "grok-4.6", "chat");
        assert_eq!(snap.active_model().unwrap().name, "grok-4.6");
        assert_eq!(
            snap.settings.voice_system_prompt,
            "Eres Ira, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta."
        );
        let cfg = engine_config(&snap);
        assert_eq!(cfg.source, "@DEFAULT_SOURCE@");
        assert!(cfg.session.barge_in);
    }
}
