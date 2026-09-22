//! Telegram HTTP transport, response decoding, and typing-indicator lifetime.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use ira_telegram::split_telegram;

#[derive(Debug, Error)]
pub enum TgError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("telegram: {0}")]
    Api(String),
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub from: Option<User>,
    pub chat: Chat,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct Chat {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: String,
}

impl Chat {
    pub fn is_private(&self) -> bool {
        self.kind == "private"
    }
}

#[derive(Deserialize)]
struct Me {
    username: Option<String>,
    first_name: String,
}

#[derive(Clone)]
pub struct Telegram {
    http: Client,
    base: String,
}

/// Keeps refreshing Telegram's typing indicator until this guard is dropped.
pub struct Typing {
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for Typing {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl Telegram {
    pub fn new(token: &str) -> Result<Self, TgError> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(40))
            .build()?;
        Ok(Self {
            http,
            base: format!("https://api.telegram.org/bot{token}"),
        })
    }

    pub async fn get_me(&self) -> Result<String, TgError> {
        let me: Me = self.get("getMe").await?;
        Ok(me.username.unwrap_or(me.first_name))
    }

    pub async fn get_updates(&self, offset: i64) -> Result<Vec<Update>, TgError> {
        let url = format!("{}/getUpdates", self.base);
        let resp = self
            .http
            .get(url)
            .query(&[
                ("offset", offset.to_string()),
                ("timeout", "25".into()),
                ("allowed_updates", r#"["message"]"#.into()),
            ])
            .send()
            .await?;
        decode(resp).await
    }

    pub async fn typing(&self, chat_id: i64) -> Result<(), TgError> {
        let url = format!("{}/sendChatAction", self.base);
        let resp = self
            .http
            .post(url)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "action": "typing",
            }))
            .send()
            .await?;
        let _: serde_json::Value = decode(resp).await?;
        Ok(())
    }

    /// Refreshes every four seconds because Telegram expires the indicator after about five.
    pub fn keep_typing(&self, chat_id: i64) -> Typing {
        let tg = self.clone();
        Typing {
            handle: tokio::spawn(async move {
                loop {
                    let _ = tg.typing(chat_id).await;
                    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                }
            }),
        }
    }

    pub async fn send_text(&self, chat_id: i64, text: &str) -> Result<(), TgError> {
        for chunk in split_telegram(text) {
            self.send_one(chat_id, &chunk).await?;
        }
        Ok(())
    }

    async fn send_one(&self, chat_id: i64, text: &str) -> Result<(), TgError> {
        #[derive(Serialize)]
        struct Body<'a> {
            chat_id: i64,
            text: &'a str,
        }
        let url = format!("{}/sendMessage", self.base);
        let resp = self
            .http
            .post(url)
            .json(&Body { chat_id, text })
            .send()
            .await?;
        let _: serde_json::Value = decode(resp).await?;
        Ok(())
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, method: &str) -> Result<T, TgError> {
        let url = format!("{}/{method}", self.base);
        let resp = self.http.get(url).send().await?;
        decode(resp).await
    }
}

async fn decode<T: for<'de> Deserialize<'de>>(resp: reqwest::Response) -> Result<T, TgError> {
    let status = resp.status();
    let body = resp.text().await?;
    let parsed: ApiResponse<T> =
        serde_json::from_str(&body).map_err(|err| TgError::Api(format!("{err}: {body}")))?;
    if !parsed.ok || !status.is_success() {
        return Err(TgError::Api(
            parsed
                .description
                .unwrap_or_else(|| format!("http {status}: {body}")),
        ));
    }
    parsed
        .result
        .ok_or_else(|| TgError::Api("respuesta sin result".into()))
}
