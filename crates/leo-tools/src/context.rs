use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Context {
    pub cwd: PathBuf,
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

    pub fn resolve(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        }
    }
}
