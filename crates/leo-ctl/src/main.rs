//! CLI adapter for the voice daemon's newline-delimited JSON protocol.
//!
//! Each invocation sends one request over the shared Unix socket, prints the
//! response state, and exits unsuccessfully on transport or daemon errors.

use clap::{Parser, Subcommand};
use leo_ipc::{Request, send, socket_path};

#[derive(Parser)]
#[command(name = "leo-ctl", about = "Controla el daemon leo-ai")]
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
