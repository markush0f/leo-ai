//! Voice control CLI (`ira-ctl`).
//!
//! One process, one command. Connects to the running `ira-daemon` Unix
//! socket, sends a single JSON request, prints `state` (and optional
//! `message`), and exits `1` on transport failure or `ok: false`. It does
//! not load the catalog, call providers, or open audio devices.
//!
//! # Workspace crates
//!
//! - [`ira_ipc`] — protocol and client. [`ira_ipc::socket_path`] is
//!   `$XDG_RUNTIME_DIR/ira-ai.sock`, else `/tmp/ira-ai.sock`. [`ira_ipc::send`]
//!   writes one newline-terminated request and reads one response. There is
//!   no client-side timeout; [`ira_ipc::IpcError::NotRunning`] means the
//!   socket is missing. Commands map 1:1 to [`ira_ipc::Request`]:
//!   - `status` — latest published session state
//!   - `listen` — enter listening (typical desktop hotkey)
//!   - `stop` — cancel listen or playback
//!   - `speak <text>` — synthesize; the daemon's current TTS fallback is a beep
//!   - `shutdown` — ask the daemon to exit
//!
//! The daemon owns session transitions. A successful send means the command
//! was accepted, not that STT/LLM/TTS has finished.

use clap::{Parser, Subcommand};
use ira_ipc::{Request, send, socket_path};

#[derive(Parser)]
#[command(name = "ira-ctl", about = "Controla el daemon ira-ai")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show the session state.
    Status,
    /// Start listening; suitable for a desktop keyboard shortcut.
    Listen,
    /// Request cancellation of listening or playback.
    Stop,
    /// Synthesize text, or play a beep when no speech model is configured.
    Speak { text: Vec<String> },
    /// Shut down the daemon.
    Shutdown,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let req = match cli.cmd {
        Cmd::Status => Request::Status,
        Cmd::Listen => Request::Listen,
        Cmd::Stop => Request::Stop,
        Cmd::Speak { text } => Request::Speak {
            text: text.join(" "),
        },
        Cmd::Shutdown => Request::Shutdown,
    };
    let resp = send(&socket_path(), &req).await?;
    if let Some(msg) = resp.message {
        println!("{} — {msg}", resp.state);
    } else {
        println!("{}", resp.state);
    }
    if !resp.ok {
        std::process::exit(1);
    }
    Ok(())
}
