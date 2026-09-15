//! Async filesystem tools returning JSON values for model consumption.
//!
//! Each operation exposes `spec` for its model-facing schema and `run` for typed
//! execution. `leo-tools` resolves user paths and adapts JSON arguments to these APIs.

/// Copies a file to a destination path.
pub mod copy_file;
mod error;
/// Lists entries in a directory.
pub mod list_directory;
/// Moves or renames a filesystem entry.
pub mod move_file;
/// Reads file contents for inclusion in a tool result.
pub mod read_file;
/// Removes files or directories according to the requested mode.
pub mod remove_file;
/// Searches filesystem entries.
pub mod search_files;
/// Writes or appends text to a file.
pub mod write_file;

pub use error::Error;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp() -> std::path::PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("leo-files-{n}"));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[tokio::test]
    async fn write_read_copy_remove() {
        let dir = tmp();
        let file = dir.join("a.txt");
        write_file::run(&file, "hola", false).await.unwrap();
        let read = read_file::run(&file, None).await.unwrap();
        assert_eq!(read["content"], "hola");
        let dest = dir.join("b.txt");
        copy_file::run(&file, &dest).await.unwrap();
        remove_file::run(&file, false).await.unwrap();
        assert!(!file.exists());
        assert!(dest.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
