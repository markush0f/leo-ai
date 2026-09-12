mod config;
mod llm;

use leo_core::{Command, LlmEngine, NullLlm, SessionEvent, spawn_engine};
use leo_ipc::{Request, Response, bind, read_request, socket_path, write_response};
use leo_llm::{Client, ProviderId};
use leo_stt::{GrokStt, NullStt, SttEngine};
use leo_tts::NullTts;
use leo_wake::load_wake;
use tokio::runtime::Handle;
use tracing_subscriber::EnvFilter;

use crate::llm::BlockingLlm;

use crate::config::{FileConfig, ensure_dirs};

#[tokio::main]
async fn main() {
    leo_llm::load_dotenv();
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
    let file = FileConfig::load();
    let engine_cfg = file.engine();
    tracing::info!(source = %engine_cfg.source, sink = %engine_cfg.sink, "leo-daemon");

    let wake = load_wake(file.wake_model().as_deref())?;
    let rt = Handle::current();
    let stt: Box<dyn SttEngine> = match std::env::var("XAI_API_KEY") {
        Ok(key) if !key.is_empty() => {
            let lang = file.stt.language.clone();
            tracing::info!(language = %lang, "stt: Grok");
            Box::new(GrokStt::new(key, rt.clone(), Some(lang)))
        }
        _ => {
            tracing::warn!("sin XAI_API_KEY: la voz no se transcribe ni llega al LLM");
            Box::new(NullStt)
        }
    };
    let llm: Box<dyn LlmEngine> = match build_llm(&file, rt) {
        Ok(llm) => llm,
        Err(err) => {
            tracing::warn!(%err, "llm deshabilitado");
            Box::new(NullLlm)
        }
    };
    let handle = spawn_engine(engine_cfg, wake, stt, llm, Box::new(NullTts))?;

    std::thread::Builder::new()
        .name("leo-events".into())
        .spawn({
            let events = handle.events;
            move || {
                while let Ok(ev) = events.recv() {
                    match ev {
                        SessionEvent::State(s) => tracing::info!(state = s.as_str(), "estado"),
                        SessionEvent::Wake { name, score } => {
                            tracing::info!(name, score, "wake")
                        }
                        SessionEvent::Transcript(t) => tracing::info!(text = %t, "voz"),
                        SessionEvent::Reply(t) => tracing::info!(text = %t, "leo"),
                        SessionEvent::BargeIn => tracing::info!("barge-in"),
                    }
                }
            }
        })?;

    let sock = socket_path();
    let listener = bind(&sock).await?;
    tracing::info!(path = %sock.display(), "socket");
    tracing::info!("hotkey: asigna un atajo a `leo-ctl listen`");

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

fn build_llm(
    file: &crate::config::FileConfig,
    rt: Handle,
) -> Result<Box<dyn LlmEngine>, Box<dyn std::error::Error>> {
    let provider = ProviderId::parse(&file.llm.provider)?;
    let mut client = Client::from_env(provider)?;
    if let Some(model) = &file.llm.model {
        client = client.with_model(model.clone());
    }
    tracing::info!(provider = %provider, "llm");
    Ok(Box::new(BlockingLlm::new(
        client,
        file.llm.system.clone(),
        rt,
    )))
}
