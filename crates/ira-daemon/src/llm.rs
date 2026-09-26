//! Bridge from the synchronous voice engine to the async provider client.

use ira_core::LlmEngine;
use ira_llm::{ChatRequest, Client};
use tokio::runtime::Handle;

/// Single-turn voice client: each request contains only system and current user text.
/// Call from a blocking thread while the associated Tokio runtime remains active.
pub struct BlockingLlm {
    client: Client,
    system: String,
    reasoning_effort: Option<String>,
    rt: Handle,
}

impl BlockingLlm {
    pub fn new(
        client: Client,
        system: String,
        reasoning_effort: Option<String>,
        rt: Handle,
    ) -> Self {
        Self {
            client,
            system,
            reasoning_effort,
            rt,
        }
    }
}

impl LlmEngine for BlockingLlm {
    fn reply(&self, user_text: &str) -> Result<String, String> {
        let mut req = ChatRequest::user(user_text).with_system(&self.system);
        req.reasoning_effort = self.reasoning_effort.clone();
        let response = self
            .rt
            .block_on(self.client.chat(req))
            .map_err(|e| e.to_string())?;
        Ok(response.text)
    }
}
