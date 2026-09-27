use std::path::{Path, PathBuf};

pub fn ira_home() -> PathBuf {
    let home = home_dir();
    let current = home.join(".ira");
    if current.exists() {
        return current;
    }
    let config = xdg_config(&home);
    for name in ["ira-ai", "leo-ai"] {
        let legacy = config.join(name);
        if !legacy.is_dir() {
            continue;
        }
        if std::fs::rename(&legacy, &current).is_ok() || current.is_dir() {
            return current;
        }
        return legacy;
    }
    current
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn xdg_config(home: &Path) -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
}
