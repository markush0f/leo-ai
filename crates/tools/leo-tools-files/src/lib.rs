pub mod copy_file;
mod error;
pub mod list_directory;
pub mod move_file;
pub mod read_file;
pub mod remove_file;
pub mod search_files;
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
