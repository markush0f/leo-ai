use leo_llm::ToolSpec;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "write_clipboard".into(),
        description: "Escribe texto en el portapapeles.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Texto a copiar" }
            },
            "required": ["text"]
        }),
    }
}

pub async fn run(text: &str) -> Result<serde_json::Value, Error> {
    if pipe("wl-copy", &[], text).await
        || pipe("xclip", &["-selection", "clipboard"], text).await
        || pipe("xsel", &["--clipboard", "--input"], text).await
    {
        return Ok(serde_json::json!({ "ok": true, "bytes": text.len() }));
    }
    Err(Error::msg("no hay wl-copy, xclip ni xsel"))
}

async fn pipe(bin: &str, args: &[&str], text: &str) -> bool {
    let mut child = match Command::new(bin)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(text.as_bytes()).await.is_err() {
            return false;
        }
    }
    child.wait().await.map(|s| s.success()).unwrap_or(false)
}
