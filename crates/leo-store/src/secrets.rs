use sqlx::{PgPool, Row};

#[derive(Debug, Clone)]
pub struct SecretRow {
    pub key: String,
    pub value: String,
}

pub async fn list_secrets(pool: &PgPool) -> Result<Vec<SecretRow>, sqlx::Error> {
    Ok(sqlx::query("SELECT key, value FROM secrets ORDER BY key")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| SecretRow {
            key: row.get("key"),
            value: row.get("value"),
        })
        .collect())
}

pub async fn get_secret(pool: &PgPool, key: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT value FROM secrets WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
}

pub async fn set_secret(pool: &PgPool, key: &str, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO secrets (key, value) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Copies stored secrets into the process environment. Database values win
/// when present; keys missing from the table keep the existing env var.
pub async fn apply_secrets_to_env(pool: &PgPool) -> Result<(), sqlx::Error> {
    for secret in list_secrets(pool).await? {
        if !secret.value.trim().is_empty() {
            unsafe {
                std::env::set_var(&secret.key, &secret.value);
            }
        }
    }
    Ok(())
}
