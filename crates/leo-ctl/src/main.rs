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
    /// Estado de la sesión
    Status,
    /// Empieza a escuchar (hotkey / “Leo”)
    Listen,
    /// Cancela la escucha o el habla
    Stop,
    /// Reproduce texto (TTS o beep si no hay modelo)
    Speak { text: Vec<String> },
    /// Apaga el daemon
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
