pub mod execute_command;
pub mod execute_script;

use std::path::Path;
use std::time::Duration;

use tokio::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn msg(m: impl Into<String>) -> Self {
        Self::Message(m.into())
    }
}

pub(crate) async fn run_command(
    mut cmd: Command,
    timeout: Duration,
) -> Result<serde_json::Value, Error> {
    let child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => {
            return Err(Error::msg(format!(
                "tiempo agotado tras {}s",
                timeout.as_secs()
            )));
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(serde_json::json!({
        "ok": output.status.success(),
        "exit_code": output.status.code(),
        "stdout": clip(&stdout, 24_000),
        "stderr": clip(&stderr, 8_000),
    }))
}

pub(crate) fn apply_cwd(cmd: &mut Command, cwd: Option<&Path>) {
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}
