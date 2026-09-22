use std::future::Future;
use std::pin::Pin;

use crate::{ChatRequest, ChatResponse, LlmError};

pub type ProviderFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ChatResponse, LlmError>> + Send + 'a>>;

/// Provider-independent completion contract.
pub trait Provider: Send + Sync {
    fn complete(&self, request: ChatRequest) -> ProviderFuture<'_>;
}
