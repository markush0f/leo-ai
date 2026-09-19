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
    #[error("falta {MASTER_KEY_ENV}; genera una clave con `openssl rand -base64 32`")]
    MissingMasterKey,
    #[error("{MASTER_KEY_ENV} debe ser base64 de exactamente 32 bytes")]
    InvalidMasterKey,
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
    pub fn from_env() -> Result<Self, DatabaseError> {
        let raw = std::env::var(MASTER_KEY_ENV).map_err(|_| DatabaseError::MissingMasterKey)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw.trim())
            .map_err(|_| DatabaseError::InvalidMasterKey)?;
        let key: [u8; 32] = bytes
            .try_into()
            .map_err(|_| DatabaseError::InvalidMasterKey)?;
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
