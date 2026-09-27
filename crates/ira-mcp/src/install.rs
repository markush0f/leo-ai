use ira_store::McpServerConfig;

use crate::error::Error;
use crate::recipe;
use crate::registry::McpRegistryClient;

pub async fn resolve(name: &str) -> Result<McpServerConfig, Error> {
    if let Some(recipe) = recipe::find(name) {
        return recipe::execute(&recipe).await;
    }
    McpRegistryClient::official().find(name).await
}
