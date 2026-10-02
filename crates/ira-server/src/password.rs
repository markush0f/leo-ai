use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

const ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub fn ensure_local_password(hash_path: &Path) -> Result<String, String> {
    let plain_path = hash_path.with_file_name("password");
    if let Some(password) = read_nonempty(&plain_path)? {
        let hash = std::fs::read_to_string(hash_path).unwrap_or_default();
        if !hash_matches(&password, hash.trim()) {
            let hash = hash_password(&password)?;
            write_secret(hash_path, hash.as_bytes())?;
        }
        return Ok(password);
    }
    let password = generate_password()?;
    let hash = hash_password(&password)?;
    write_secret(&plain_path, password.as_bytes())?;
    write_secret(hash_path, hash.as_bytes())?;
    Ok(password)
}

fn read_nonempty(path: &Path) -> Result<Option<String>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    Ok(Some(raw.to_string()))
}

fn generate_password() -> Result<String, String> {
    let mut bytes = [0u8; 12];
    getrandom::fill(&mut bytes).map_err(|err| err.to_string())?;
    Ok(bytes
        .into_iter()
        .map(|byte| ALPHABET[(byte as usize) % ALPHABET.len()] as char)
        .collect())
}

fn hash_password(password: &str) -> Result<String, String> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|err| err.to_string())?;
    let salt = SaltString::encode_b64(&salt).map_err(|err| err.to_string())?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| err.to_string())
}

fn write_secret(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|err| err.to_string())?;
    file.write_all(bytes).map_err(|err| err.to_string())?;
    file.write_all(b"\n").map_err(|err| err.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn hash_matches(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}
