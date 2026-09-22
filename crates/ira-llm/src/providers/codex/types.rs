use serde::{Deserialize, Serialize};

use super::CODEX_API_ENDPOINT;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OAuthCredentials {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix timestamp in milliseconds.
    pub expires_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CodexConfig {
    pub endpoint: String,
    pub default_model: String,
    pub originator: String,
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            endpoint: CODEX_API_ENDPOINT.into(),
            default_model: "gpt-5.4".into(),
            originator: "ira-ai".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceAuthorization {
    pub device_auth_id: String,
    pub user_code: String,
    pub interval: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
}
