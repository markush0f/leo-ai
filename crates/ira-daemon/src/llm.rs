//! Bridge from the synchronous voice engine to the async provider client.

use ira_core::LlmEngine;
use ira_llm::{ChatRequest, Client};
use tokio::runtime::Handle;

/// Single-turn voice client: each request contains only system and current user text.
/// Call from a blocking thread while the associated Tokio runtime remains active.
pub struct BlockingLlm {
    client: Client,
    pool: sqlx::PgPool,
    rt: Handle,
}

impl BlockingLlm {
    pub fn new(client: Client, pool: sqlx::PgPool, rt: Handle) -> Self {
        Self { client, pool, rt }
    }
}

impl LlmEngine for BlockingLlm {
    fn reply(&self, user_text: &str) -> Result<String, String> {
        let (system, effort) = self.rt.block_on(async {
            let snap = ira_store::load(&self.pool).await.map_err(|e| e.to_string())?;
            let model = snap
                .active_model()
                .map(|model| model.name.clone())
                .unwrap_or_else(|| "desconocido".into());
            let memory = ira_store::prepare_turn(&self.pool, user_text)
                .await
                .map_err(|e| e.to_string())?;
            let system = ira_store::compose_prompt(
                &self.pool,
                "voice",
                &ira_store::PromptDynamic {
                    model: &model,
                    tool_names: &[],
                    memories: &memory.prompt,
                    extra: "",
                },
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok::<_, String>((system, snap.reasoning_effort()))
        })?;
        let mut req = ChatRequest::user(user_text).with_system(&system);
        req.reasoning_effort = effort;
        let response = self
            .rt
            .block_on(self.client.chat(req))
            .map_err(|e| e.to_string())?;
        Ok(response.text)
    }
}
