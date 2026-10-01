use std::io::{self, Read};

use argon2::password_hash::{PasswordHasher, SaltString};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut password = String::new();
    io::stdin().read_to_string(&mut password)?;
    let password = password.trim_end_matches(['\r', '\n']);
    if password.is_empty() {
        return Err("contraseña vacía".into());
    }
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|err| format!("salt: {err}"))?;
    let salt = SaltString::encode_b64(&salt).map_err(|err| format!("salt: {err}"))?;
    let hash = argon2::Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| format!("hash: {err}"))?;
    println!("{hash}");
    Ok(())
}
