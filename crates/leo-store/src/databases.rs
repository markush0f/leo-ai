use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use sqlx::{PgPool, Row};
use uuid::Uuid;

const MASTER_KEY_ENV: &str = "LEO_MASTER_KEY";

#[derive(Clone)]
pub struct DatabaseCipher([u8; 32]);

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error("{MASTER_KEY_ENV} debe ser base64 de exactamente 32 bytes")]
    InvalidMasterKey,
    #[error("la clave guardada en {path} no es válida")]
    InvalidStoredKey { path: String },
    #[error("no se pudo generar la clave maestra")]
    Generate,
    #[error("no se pudo guardar la clave maestra en {path}: {source}")]
    MasterKeyStore {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("no se pudo cifrar la contraseña")]
    Encrypt,
    #[error("no se pudo descifrar la contraseña; comprueba {MASTER_KEY_ENV}")]
    Decrypt,
    #[error("conexión de base de datos inexistente")]
    NotFound,
}

#[derive(Debug, Clone)]
pub struct DatabaseConnectionRow {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub database: String,
    pub username: String,
    pub ssl_mode: String,
    pub enabled: bool,
    pub password_set: bool,
    pub last_test_ok: Option<bool>,
    pub last_test_error: Option<String>,
    pub last_tested_at: Option<String>,
}

pub struct DatabaseWrite {
    pub name: String,
    pub host: String,
    pub port: i32,
    pub database: String,
    pub username: String,
    pub password: Option<String>,
    pub ssl_mode: String,
    pub enabled: bool,
}

impl DatabaseCipher {
    /// Usa `LEO_MASTER_KEY` si existe. Si no, crea `.leo/master.key` y la reutiliza.
    pub fn from_env() -> Result<Self, DatabaseError> {
        if let Ok(raw) = std::env::var(MASTER_KEY_ENV) {
            let raw = raw.trim();
            if !raw.is_empty() {
                return Self::parse(raw).map_err(|_| DatabaseError::InvalidMasterKey);
            }
        }
        Ok(ensure_master_key(&master_key_path())?.0)
    }

    fn parse(raw: &str) -> Result<Self, ()> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw.trim())
            .map_err(|_| ())?;
        let key: [u8; 32] = bytes.try_into().map_err(|_| ())?;
        Ok(Self(key))
    }

    fn encrypt(&self, id: Uuid, password: &str) -> Result<(Vec<u8>, Vec<u8>), DatabaseError> {
        let mut nonce = [0_u8; 24];
        getrandom::fill(&mut nonce).map_err(|_| DatabaseError::Encrypt)?;
        let cipher = XChaCha20Poly1305::new((&self.0).into());
        let encrypted = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: password.as_bytes(),
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| DatabaseError::Encrypt)?;
        Ok((encrypted, nonce.to_vec()))
    }

    fn decrypt(&self, id: Uuid, encrypted: &[u8], nonce: &[u8]) -> Result<String, DatabaseError> {
        if nonce.len() != 24 {
            return Err(DatabaseError::Decrypt);
        }
        let cipher = XChaCha20Poly1305::new((&self.0).into());
        let plain = cipher
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: encrypted,
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| DatabaseError::Decrypt)?;
        String::from_utf8(plain).map_err(|_| DatabaseError::Decrypt)
    }
}

pub async fn list_database_connections(
    pool: &PgPool,
) -> Result<Vec<DatabaseConnectionRow>, DatabaseError> {
    let rows = sqlx::query(
        "SELECT id, name, host, port, database_name, username, ssl_mode, enabled,
                password_ciphertext IS NOT NULL AS password_set,
                last_test_ok, last_test_error,
                last_tested_at::text AS last_tested_at
         FROM database_connections ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_from_sql).collect())
}

pub async fn database_connection(
    pool: &PgPool,
    id: Uuid,
) -> Result<DatabaseConnectionRow, DatabaseError> {
    let row = sqlx::query(
        "SELECT id, name, host, port, database_name, username, ssl_mode, enabled,
                password_ciphertext IS NOT NULL AS password_set,
                last_test_ok, last_test_error,
                last_tested_at::text AS last_tested_at
         FROM database_connections WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(DatabaseError::NotFound)?;
    Ok(row_from_sql(row))
}

pub async fn create_database_connection(
    pool: &PgPool,
    cipher: &DatabaseCipher,
    input: DatabaseWrite,
) -> Result<DatabaseConnectionRow, DatabaseError> {
    let id = Uuid::new_v4();
    let password = input.password.as_deref().filter(|value| !value.is_empty());
    let (encrypted, nonce) = match password {
        Some(password) => {
            let (encrypted, nonce) = cipher.encrypt(id, password)?;
            (Some(encrypted), Some(nonce))
        }
        None => (None, None),
    };
    sqlx::query(
        "INSERT INTO database_connections
         (id, name, host, port, database_name, username, ssl_mode, enabled,
          password_ciphertext, password_nonce)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(id)
    .bind(input.name)
    .bind(input.host)
    .bind(input.port)
    .bind(input.database)
    .bind(input.username)
    .bind(input.ssl_mode)
    .bind(input.enabled)
    .bind(encrypted)
    .bind(nonce)
    .execute(pool)
    .await?;
    database_connection(pool, id).await
}

pub async fn update_database_connection(
    pool: &PgPool,
    cipher: &DatabaseCipher,
    id: Uuid,
    input: DatabaseWrite,
) -> Result<DatabaseConnectionRow, DatabaseError> {
    let password = input.password.as_deref().filter(|value| !value.is_empty());
    let encrypted = password
        .map(|password| cipher.encrypt(id, password))
        .transpose()?;
    let result = if let Some((encrypted, nonce)) = encrypted {
        sqlx::query(
            "UPDATE database_connections
             SET name = $2, host = $3, port = $4, database_name = $5, username = $6,
                 ssl_mode = $7, enabled = $8, password_ciphertext = $9,
                 password_nonce = $10, last_test_ok = NULL, last_test_error = NULL,
                 last_tested_at = NULL, updated_at = now()
             WHERE id = $1",
        )
        .bind(id)
        .bind(input.name)
        .bind(input.host)
        .bind(input.port)
        .bind(input.database)
        .bind(input.username)
        .bind(input.ssl_mode)
        .bind(input.enabled)
        .bind(encrypted)
        .bind(nonce)
        .execute(pool)
        .await?
    } else {
        sqlx::query(
            "UPDATE database_connections
             SET name = $2, host = $3, port = $4, database_name = $5, username = $6,
                 ssl_mode = $7, enabled = $8, last_test_ok = NULL,
                 last_test_error = NULL, last_tested_at = NULL, updated_at = now()
             WHERE id = $1",
        )
        .bind(id)
        .bind(input.name)
        .bind(input.host)
        .bind(input.port)
        .bind(input.database)
        .bind(input.username)
        .bind(input.ssl_mode)
        .bind(input.enabled)
        .execute(pool)
        .await?
    };
    if result.rows_affected() == 0 {
        return Err(DatabaseError::NotFound);
    }
    database_connection(pool, id).await
}

pub async fn delete_database_connection(pool: &PgPool, id: Uuid) -> Result<(), DatabaseError> {
    let result = sqlx::query("DELETE FROM database_connections WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(DatabaseError::NotFound);
    }
    Ok(())
}

pub async fn database_password(
    pool: &PgPool,
    cipher: &DatabaseCipher,
    id: Uuid,
) -> Result<String, DatabaseError> {
    let row = sqlx::query(
        "SELECT password_ciphertext, password_nonce FROM database_connections WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(DatabaseError::NotFound)?;
    let encrypted: Option<Vec<u8>> = row.get("password_ciphertext");
    let nonce: Option<Vec<u8>> = row.get("password_nonce");
    match (encrypted, nonce) {
        (Some(encrypted), Some(nonce)) => cipher.decrypt(id, &encrypted, &nonce),
        _ => Ok(String::new()),
    }
}

pub async fn set_database_test_result(
    pool: &PgPool,
    id: Uuid,
    ok: bool,
    error: Option<&str>,
) -> Result<(), DatabaseError> {
    sqlx::query(
        "UPDATE database_connections
         SET last_test_ok = $2, last_test_error = $3, last_tested_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(ok)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

fn master_key_path() -> PathBuf {
    let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = start.clone();
    for _ in 0..16 {
        if dir.join("Cargo.toml").is_file() && dir.join("crates").is_dir() {
            return dir.join(".leo/master.key");
        }
        if !dir.pop() {
            break;
        }
    }
    start.join(".leo/master.key")
}

fn ensure_master_key(path: &Path) -> Result<(DatabaseCipher, String), DatabaseError> {
    if path.is_file() {
        return read_master_key(path);
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|source| store_error(path, source))?;
    }
    let mut key = [0_u8; 32];
    getrandom::fill(&mut key).map_err(|_| DatabaseError::Generate)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(key);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(encoded.as_bytes())
                .and_then(|()| file.write_all(b"\n"))
                .map_err(|source| store_error(path, source))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|source| store_error(path, source))?;
            }
            Ok((DatabaseCipher(key), encoded))
        }
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => read_master_key(path),
        Err(source) => Err(store_error(path, source)),
    }
}

fn read_master_key(path: &Path) -> Result<(DatabaseCipher, String), DatabaseError> {
    let raw = std::fs::read_to_string(path).map_err(|source| store_error(path, source))?;
    let encoded = raw.trim().to_string();
    let cipher = DatabaseCipher::parse(&encoded).map_err(|_| DatabaseError::InvalidStoredKey {
        path: path.display().to_string(),
    })?;
    Ok((cipher, encoded))
}

fn store_error(path: &Path, source: std::io::Error) -> DatabaseError {
    DatabaseError::MasterKeyStore {
        path: path.display().to_string(),
        source,
    }
}

fn row_from_sql(row: sqlx::postgres::PgRow) -> DatabaseConnectionRow {
    DatabaseConnectionRow {
        id: row.get("id"),
        name: row.get("name"),
        host: row.get("host"),
        port: row.get("port"),
        database: row.get("database_name"),
        username: row.get("username"),
        ssl_mode: row.get("ssl_mode"),
        enabled: row.get("enabled"),
        password_set: row.get("password_set"),
        last_test_ok: row.get("last_test_ok"),
        last_test_error: row.get("last_test_error"),
        last_tested_at: row.get("last_tested_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn creates_a_stable_master_key_without_env() {
        let root = std::env::temp_dir().join(format!("leo-master-{}", Uuid::new_v4()));
        let path = root.join("master.key");
        let (first, encoded) = ensure_master_key(&path).unwrap();
        let (second, again) = ensure_master_key(&path).unwrap();
        assert_eq!(encoded, again);
        assert_eq!(first.0, second.0);
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn env_key_overrides_a_stored_file() {
        let encoded = base64::engine::general_purpose::STANDARD.encode([3_u8; 32]);
        let previous = std::env::var(MASTER_KEY_ENV).ok();
        unsafe { std::env::set_var(MASTER_KEY_ENV, &encoded) };
        let cipher = DatabaseCipher::from_env().unwrap();
        match previous {
            Some(value) => unsafe { std::env::set_var(MASTER_KEY_ENV, value) },
            None => unsafe { std::env::remove_var(MASTER_KEY_ENV) },
        }
        assert_eq!(cipher.0, [3_u8; 32]);
    }

    #[test]
    fn cipher_round_trip_and_binds_id() {
        let cipher = DatabaseCipher([7; 32]);
        let id = Uuid::new_v4();
        let (encrypted, nonce) = cipher.encrypt(id, "secret").unwrap();
        assert_ne!(encrypted, b"secret");
        assert_eq!(cipher.decrypt(id, &encrypted, &nonce).unwrap(), "secret");
        assert!(cipher.decrypt(Uuid::new_v4(), &encrypted, &nonce).is_err());
    }

    #[tokio::test]
    async fn stores_ciphertext_and_preserves_password_on_update() {
        let Ok(pool) = crate::connect(&crate::database_url()).await else {
            return;
        };
        crate::migrate(&pool).await.unwrap();
        let cipher = DatabaseCipher([9; 32]);
        let name = format!("test-{}", Uuid::new_v4());
        let row = create_database_connection(
            &pool,
            &cipher,
            DatabaseWrite {
                name: name.clone(),
                host: "127.0.0.1".into(),
                port: 5432,
                database: "example".into(),
                username: "reader".into(),
                password: Some("secret".into()),
                ssl_mode: "disable".into(),
                enabled: false,
            },
        )
        .await
        .unwrap();
        assert!(row.password_set);
        assert_eq!(
            database_password(&pool, &cipher, row.id).await.unwrap(),
            "secret"
        );

        let row = update_database_connection(
            &pool,
            &cipher,
            row.id,
            DatabaseWrite {
                name,
                host: "db.internal".into(),
                port: 5433,
                database: "example".into(),
                username: "reader".into(),
                password: None,
                ssl_mode: "require".into(),
                enabled: false,
            },
        )
        .await
        .unwrap();
        assert_eq!(row.host, "db.internal");
        assert_eq!(
            database_password(&pool, &cipher, row.id).await.unwrap(),
            "secret"
        );
        delete_database_connection(&pool, row.id).await.unwrap();
    }
}
