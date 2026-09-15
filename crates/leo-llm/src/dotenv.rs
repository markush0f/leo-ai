use std::path::{Path, PathBuf};

/// Loads the first `.env` found while walking up from the working directory.
/// Checks at most 16 directories, preserves exported variables, and ignores load errors.
pub fn load_dotenv() {
    if let Some(path) = find_dotenv() {
        let _ = dotenvy::from_path(&path);
    }
}

fn find_dotenv() -> Option<PathBuf> {
    find_dotenv_from(&std::env::current_dir().ok()?)
}

pub(crate) fn find_dotenv_from(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    for _ in 0..16 {
        let path = dir.join(".env");
        if path.is_file() {
            return Some(path);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_env_in_parent() {
        let root = std::env::temp_dir().join(format!("leo-dotenv-{}", std::process::id()));
        let nested = root.join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        let env_path = root.join(".env");
        std::fs::write(&env_path, "FOO=bar\n").unwrap();
        let found = find_dotenv_from(&nested).expect("parent .env");
        assert_eq!(found, env_path);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_env_is_none() {
        let root = std::env::temp_dir().join(format!("leo-dotenv-none-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(find_dotenv_from(&root).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
