use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::OAuthCredentials;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct TokenStoreError(pub String);

pub type TokenStoreFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, TokenStoreError>> + Send + 'a>>;

/// Storage boundary for OAuth credentials. Implementations may use Postgres,
/// an encrypted secret service, or local memory.
pub trait TokenStore: Send + Sync {
    fn load(&self) -> TokenStoreFuture<'_, Option<OAuthCredentials>>;
    fn save(&self, credentials: &OAuthCredentials) -> TokenStoreFuture<'_, ()>;
}

#[derive(Clone, Default)]
pub struct MemoryTokenStore {
    credentials: Arc<RwLock<Option<OAuthCredentials>>>,
}

impl MemoryTokenStore {
    pub fn new(credentials: OAuthCredentials) -> Self {
        Self {
            credentials: Arc::new(RwLock::new(Some(credentials))),
        }
    }
}

impl TokenStore for MemoryTokenStore {
    fn load(&self) -> TokenStoreFuture<'_, Option<OAuthCredentials>> {
        Box::pin(async { Ok(self.credentials.read().await.clone()) })
    }

    fn save(&self, credentials: &OAuthCredentials) -> TokenStoreFuture<'_, ()> {
        let credentials = credentials.clone();
        Box::pin(async move {
            *self.credentials.write().await = Some(credentials);
            Ok(())
        })
    }
}
