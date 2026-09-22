use ira_llm::ToolSpec;
use tokio::process::Command;

use crate::error::Error;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "read_clipboard".into(),
        description: "Lee el texto del portapapeles (wl-paste, xclip o xsel).".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub async fn run() -> Result<serde_json::Value, Error> {
    let text = try_cmd("wl-paste", &[])
        .await
        .or(try_cmd("xclip", &["-selection", "clipboard", "-o"]).await)
        .or(try_cmd("xsel", &["--clipboard", "--output"]).await)
        .ok_or_else(|| Error::msg("no hay wl-paste, xclip ni xsel"))?;
    Ok(serde_json::json!({
        "ok": true,
        "text": crate::error::clip(&text, 20_000),
    }))
}

async fn try_cmd(bin: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(bin).args(args).output().await.ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}
