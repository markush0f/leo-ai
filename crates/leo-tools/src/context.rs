use std::path::{Path, PathBuf};

/// Execution context captured when the registry is built.
#[derive(Clone)]
pub struct Context {
    /// Base directory for relative tool paths; not a filesystem sandbox.
    pub cwd: PathBuf,
    /// Reusable HTTP client for integrations that do not own a dedicated client.
    pub http: reqwest::Client,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            http: reqwest::Client::new(),
        }
    }
}

impl Context {
    pub fn from_env() -> Self {
        Self::default()
    }

    /// Joins relative paths to `cwd` and leaves absolute paths unchanged.
    /// Does not canonicalize paths or reject `..` components.
    pub fn resolve(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        }
    }
}
