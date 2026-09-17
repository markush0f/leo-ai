use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::LlmError;

use super::token_store::TokenStore;
use super::types::{DeviceAuthorization, OAuthCredentials, TokenResponse};

pub const OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const OAUTH_ISSUER: &str = "https://auth.openai.com";
pub const CODEX_API_ENDPOINT: &str = "https://chatgpt.com/backend-api/codex/responses";
const BROWSER_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";

#[derive(Debug, Clone)]
pub struct BrowserLogin {
    pub authorization_url: String,
    pub state: String,
    code_verifier: String,
}

#[derive(Debug, Clone)]
pub struct DeviceLogin {
    pub verification_url: String,
    pub authorization: DeviceAuthorization,
}

#[derive(Clone)]
pub struct OAuthClient {
    http: reqwest::Client,
    issuer: String,
    client_id: String,
}

impl Default for OAuthClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OAuthClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            issuer: OAUTH_ISSUER.into(),
            client_id: OAUTH_CLIENT_ID.into(),
        }
    }

    pub fn browser_login(&self) -> Result<BrowserLogin, LlmError> {
        let code_verifier = random_urlsafe(32)?;
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
        let state = random_urlsafe(32)?;
        let query = [
            ("response_type", "code".to_string()),
            ("client_id", self.client_id.clone()),
            ("redirect_uri", BROWSER_REDIRECT_URI.into()),
            ("scope", "openid profile email offline_access".into()),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256".into()),
            ("id_token_add_organizations", "true".into()),
            ("codex_cli_simplified_flow", "true".into()),
            ("state", state.clone()),
            ("originator", "leo-ai".into()),
        ];
        let authorization_url =
            reqwest::Url::parse_with_params(&format!("{}/oauth/authorize", self.issuer), query)?
                .to_string();
        Ok(BrowserLogin {
            authorization_url,
            state,
            code_verifier,
        })
    }

    pub async fn exchange_browser_code(
        &self,
        login: &BrowserLogin,
        code: &str,
        state: &str,
    ) -> Result<OAuthCredentials, LlmError> {
        if state != login.state {
            return Err(LlmError::Authentication("estado OAuth inválido".into()));
        }
        self.exchange_code(code, &login.code_verifier, BROWSER_REDIRECT_URI)
            .await
    }

    pub async fn begin_device_login(&self) -> Result<DeviceLogin, LlmError> {
        let response = self
            .http
            .post(format!("{}/api/accounts/deviceauth/usercode", self.issuer))
            .header("User-Agent", "leo-ai")
            .json(&serde_json::json!({ "client_id": self.client_id }))
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(oauth_http_error(status.as_u16(), body));
        }
        Ok(DeviceLogin {
            verification_url: format!("{}/codex/device", self.issuer),
            authorization: serde_json::from_str(&body)?,
        })
    }

    /// Polls until authorization succeeds. A caller can cancel this future.
    pub async fn finish_device_login(
        &self,
        login: &DeviceLogin,
    ) -> Result<OAuthCredentials, LlmError> {
        #[derive(Deserialize)]
        struct DeviceToken {
            authorization_code: String,
            code_verifier: String,
        }

        let seconds = login
            .authorization
            .interval
            .parse::<u64>()
            .unwrap_or(5)
            .max(1);
        loop {
            let response = self
                .http
                .post(format!("{}/api/accounts/deviceauth/token", self.issuer))
                .header("User-Agent", "leo-ai")
                .json(&serde_json::json!({
                    "device_auth_id": login.authorization.device_auth_id,
                    "user_code": login.authorization.user_code,
                }))
                .send()
                .await?;
            if response.status().is_success() {
                let token: DeviceToken = response.json().await?;
                return self
                    .exchange_code(
                        &token.authorization_code,
                        &token.code_verifier,
                        DEVICE_REDIRECT_URI,
                    )
                    .await;
            }
            if !matches!(response.status().as_u16(), 403 | 404) {
                return Err(oauth_http_error(
                    response.status().as_u16(),
                    response.text().await?,
                ));
            }
            tokio::time::sleep(Duration::from_secs(seconds + 3)).await;
        }
    }

    pub async fn refresh(
        &self,
        credentials: &OAuthCredentials,
    ) -> Result<OAuthCredentials, LlmError> {
        let response = self
            .http
            .post(format!("{}/oauth/token", self.issuer))
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", credentials.refresh_token.as_str()),
                ("client_id", self.client_id.as_str()),
            ])
            .send()
            .await?;
        self.credentials_from_response(response, Some(credentials))
            .await
    }

    async fn exchange_code(
        &self,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> Result<OAuthCredentials, LlmError> {
        let response = self
            .http
            .post(format!("{}/oauth/token", self.issuer))
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("client_id", self.client_id.as_str()),
                ("code_verifier", verifier),
            ])
            .send()
            .await?;
        self.credentials_from_response(response, None).await
    }

    async fn credentials_from_response(
        &self,
        response: reqwest::Response,
        previous: Option<&OAuthCredentials>,
    ) -> Result<OAuthCredentials, LlmError> {
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(oauth_http_error(status.as_u16(), body));
        }
        let token: TokenResponse = serde_json::from_str(&body)?;
        let account_id = token
            .id_token
            .as_deref()
            .and_then(account_id_from_token)
            .or_else(|| account_id_from_token(&token.access_token))
            .or_else(|| previous.and_then(|p| p.account_id.clone()));
        Ok(OAuthCredentials {
            access_token: token.access_token,
            refresh_token: token
                .refresh_token
                .or_else(|| previous.map(|p| p.refresh_token.clone()))
                .ok_or_else(|| {
                    LlmError::Authentication("OAuth no devolvió refresh_token".into())
                })?,
            expires_at: now_ms().saturating_add(token.expires_in.unwrap_or(3600) * 1000),
            account_id,
        })
    }
}

#[derive(Clone)]
pub struct AuthManager {
    oauth: OAuthClient,
    store: Arc<dyn TokenStore>,
    refresh_lock: Arc<Mutex<()>>,
}

impl AuthManager {
    pub fn new(store: Arc<dyn TokenStore>) -> Self {
        Self {
            oauth: OAuthClient::new(),
            store,
            refresh_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn authorization(&self) -> Result<OAuthCredentials, LlmError> {
        let credentials = self.load().await?;
        if credentials.expires_at > now_ms().saturating_add(30_000) {
            return Ok(credentials);
        }

        let _guard = self.refresh_lock.lock().await;
        let credentials = self.load().await?;
        if credentials.expires_at > now_ms().saturating_add(30_000) {
            return Ok(credentials);
        }
        let refreshed = match self.oauth.refresh(&credentials).await {
            Ok(refreshed) => refreshed,
            Err(error) => {
                let current = self.load().await?;
                if current.access_token != credentials.access_token {
                    return Ok(current);
                }
                return Err(error);
            }
        };
        self.store
            .save(&refreshed)
            .await
            .map_err(|e| LlmError::TokenStore(e.to_string()))?;
        Ok(refreshed)
    }

    pub async fn refresh_after(
        &self,
        rejected_access_token: &str,
    ) -> Result<OAuthCredentials, LlmError> {
        let _guard = self.refresh_lock.lock().await;
        let credentials = self.load().await?;
        if credentials.access_token != rejected_access_token {
            return Ok(credentials);
        }
        let refreshed = self.oauth.refresh(&credentials).await?;
        self.store
            .save(&refreshed)
            .await
            .map_err(|e| LlmError::TokenStore(e.to_string()))?;
        Ok(refreshed)
    }

    async fn load(&self) -> Result<OAuthCredentials, LlmError> {
        self.store
            .load()
            .await
            .map_err(|e| LlmError::TokenStore(e.to_string()))?
            .ok_or(LlmError::MissingCredentials("codex"))
    }
}

fn random_urlsafe(bytes: usize) -> Result<String, LlmError> {
    let mut value = vec![0; bytes];
    getrandom::fill(&mut value).map_err(|e| LlmError::Authentication(e.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(value))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn oauth_http_error(status: u16, body: String) -> LlmError {
    LlmError::Http {
        status,
        body: crate::extract_error_message(&body).unwrap_or(body),
    }
}

fn account_id_from_token(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .get("chatgpt_account_id")
        .and_then(|v| v.as_str())
        .or_else(|| {
            claims
                .get("https://api.openai.com/auth")
                .and_then(|v| v.get("chatgpt_account_id"))
                .and_then(|v| v.as_str())
        })
        .or_else(|| {
            claims
                .get("organizations")
                .and_then(|v| v.as_array())
                .and_then(|v| v.first())
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str())
        })
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_login_uses_pkce_and_expected_parameters() {
        let login = OAuthClient::new().browser_login().unwrap();
        let url = reqwest::Url::parse(&login.authorization_url).unwrap();
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query.get("client_id").unwrap(), OAUTH_CLIENT_ID);
        assert_eq!(query.get("code_challenge_method").unwrap(), "S256");
        assert_eq!(query.get("state").unwrap(), &login.state);
        assert!(!login.code_verifier.is_empty());
    }

    #[test]
    fn extracts_namespaced_account_id() {
        let payload = URL_SAFE_NO_PAD
            .encode(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acc_123"}}"#);
        assert_eq!(
            account_id_from_token(&format!("x.{payload}.y")).as_deref(),
            Some("acc_123")
        );
    }
}
