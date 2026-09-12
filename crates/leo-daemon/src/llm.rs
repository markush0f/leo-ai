use leo_core::LlmEngine;
use leo_llm::{ChatRequest, Client};
use tokio::runtime::Handle;

pub struct BlockingLlm {
    client: Client,
    system: String,
    rt: Handle,
}

impl BlockingLlm {
    pub fn new(client: Client, system: String, rt: Handle) -> Self {
        Self { client, system, rt }
    }
}

impl LlmEngine for BlockingLlm {
    fn reply(&self, user_text: &str) -> Result<String, String> {
        let req = ChatRequest::user(user_text).with_system(&self.system);
        let response = self
            .rt
            .block_on(self.client.chat(req))
            .map_err(|e| e.to_string())?;
        Ok(response.text)
    }
}
