use std::sync::Arc;

use ira_llm::providers::codex::{OAuthCredentials, TokenStore, TokenStoreError, TokenStoreFuture};
use ira_llm::{Client, LlmError};
use sqlx::PgPool;
use uuid::Uuid;

use crate::Snapshot;

#[derive(Clone)]
pub struct PostgresCodexTokenStore {
    pool: PgPool,
    provider_id: Uuid,
}

impl PostgresCodexTokenStore {
    pub fn new(pool: PgPool, provider_id: Uuid) -> Self {
        Self { pool, provider_id }
    }
}

impl TokenStore for PostgresCodexTokenStore {
    fn load(&self) -> TokenStoreFuture<'_, Option<OAuthCredentials>> {
        Box::pin(async move {
            let encoded: Option<String> =
                sqlx::query_scalar("SELECT api_key FROM providers WHERE id = $1")
                    .bind(self.provider_id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(store_error)?
                    .flatten();
            encoded
                .filter(|value| !value.trim().is_empty())
                .map(|value| serde_json::from_str(&value).map_err(store_error))
                .transpose()
        })
    }

    fn save(&self, credentials: &OAuthCredentials) -> TokenStoreFuture<'_, ()> {
        let credentials = credentials.clone();
        Box::pin(async move {
            let encoded = serde_json::to_string(&credentials).map_err(store_error)?;
            let result = sqlx::query("UPDATE providers SET api_key = $1 WHERE id = $2")
                .bind(encoded)
                .bind(self.provider_id)
                .execute(&self.pool)
                .await
                .map_err(store_error)?;
            if result.rows_affected() == 0 {
                return Err(TokenStoreError("proveedor Codex inexistente".into()));
            }
            Ok(())
        })
    }
}

pub fn client_with_pool(snapshot: &Snapshot, pool: &PgPool) -> Result<Client, LlmError> {
    let model = snapshot.active_model().ok_or(LlmError::Empty("modelo"))?;
    let provider = snapshot
        .active_provider()
        .ok_or(LlmError::Empty("proveedor"))?;
    if !provider.kind.eq_ignore_ascii_case("codex") {
        return snapshot.client();
    }

    let store = Arc::new(PostgresCodexTokenStore::new(pool.clone(), provider.id));
    let mut client = Client::codex(store).with_model(model.name.clone());
    if let Some(url) = provider
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        client = client.with_base_url(url);
    }
    Ok(client)
}

pub async fn sync_codex_providers(pool: &PgPool) -> Result<(), String> {
    let snapshot = crate::load(pool).await.map_err(|e| e.to_string())?;
    for provider in snapshot.providers {
        if !provider.kind.eq_ignore_ascii_case("codex") || provider.api_key.is_none() {
            continue;
        }
        sync_codex_provider(pool, provider.id).await?;
    }
    Ok(())
}

pub async fn sync_codex_provider(pool: &PgPool, provider_id: Uuid) -> Result<(), String> {
    let store = Arc::new(PostgresCodexTokenStore::new(pool.clone(), provider_id));
    let names = Client::codex(store)
        .list_models()
        .await
        .map_err(|e| e.to_string())?;
    if names.is_empty() {
        return Err("Codex no devolvió modelos disponibles para esta cuenta".into());
    }
    crate::replace_models(pool, provider_id, &names)
        .await
        .map_err(|e| e.to_string())
}

fn store_error(error: impl std::fmt::Display) -> TokenStoreError {
    TokenStoreError(error.to_string())
}
