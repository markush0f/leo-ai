use std::time::Duration;

use crate::error::{Error, clip, env, redact};

const ENDPOINT: &str = "https://api.callmebot.com/whatsapp.php";
pub(crate) const MAX_TEXT: usize = 900;

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    phone: String,
    apikey: String,
}

impl Client {
    pub fn from_env() -> Option<Self> {
        let phone = env("CALLMEBOT_PHONE")?;
        let apikey = env("CALLMEBOT_APIKEY")?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Some(Self {
            http,
            phone,
            apikey,
        })
    }

    pub async fn send(&self, text: &str) -> Result<(), Error> {
        let phone = normalize_phone(&self.phone)?;
        let text = clip_text(text)?;
        let url = whatsapp_url(&phone, &text, &self.apikey)?;
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|_| Error::msg("no se pudo contactar CallMeBot"))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|_| Error::msg("CallMeBot no devolvió cuerpo"))?;
        let body = redact(&body, &self.apikey);
        if upstream_failed(status, &body) {
            return Err(Error::Http {
                status,
                body: clip(&body),
            });
        }
        Ok(())
    }
}

pub(crate) fn normalize_phone(raw: &str) -> Result<String, Error> {
    let phone: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    let digits = phone.chars().filter(|c| c.is_ascii_digit()).count();
    let plus_ok =
        phone.matches('+').count() <= 1 && (!phone.contains('+') || phone.starts_with('+'));
    let chars_ok = phone.chars().all(|c| c.is_ascii_digit() || c == '+');
    if phone.is_empty() || digits < 8 || !plus_ok || !chars_ok {
        return Err(Error::msg(
            "CALLMEBOT_PHONE inválido: usa el número con prefijo, por ejemplo +34600000000",
        ));
    }
    Ok(phone)
}

pub(crate) fn clip_text(text: &str) -> Result<String, Error> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::msg("falta text"));
    }
    Ok(text.chars().take(MAX_TEXT).collect())
}

pub(crate) fn whatsapp_url(phone: &str, text: &str, apikey: &str) -> Result<String, Error> {
    let mut url = url::Url::parse(ENDPOINT).map_err(|_| Error::msg("url de CallMeBot inválida"))?;
    url.query_pairs_mut()
        .append_pair("phone", phone)
        .append_pair("text", text)
        .append_pair("apikey", apikey);
    Ok(url.into())
}

fn upstream_failed(status: u16, body: &str) -> bool {
    if !(200..300).contains(&status) {
        return true;
    }
    let t = body.trim().to_ascii_lowercase();
    t.starts_with("error") || t.contains("invalid apikey") || t.contains("invalid api key")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encodes_text_and_keeps_phone() {
        let url = whatsapp_url("+34600111222", "hola ira\nlinea", "secret-key").unwrap();
        assert!(url.starts_with("https://api.callmebot.com/whatsapp.php?"));
        assert!(url.contains("phone=%2B34600111222"));
        assert!(url.contains("text=hola+ira%0Alinea") || url.contains("text=hola%20ira%0Alinea"));
        assert!(url.contains("apikey=secret-key"));
        assert!(!url.contains('\n'));
    }

    #[test]
    fn clips_long_text_and_rejects_empty() {
        let long = "á".repeat(MAX_TEXT + 20);
        let clipped = clip_text(&long).unwrap();
        assert_eq!(clipped.chars().count(), MAX_TEXT);
        assert!(clip_text("   ").is_err());
    }

    #[test]
    fn normalizes_phone() {
        assert_eq!(
            normalize_phone(" +34 600 111 222 ").unwrap(),
            "+34600111222"
        );
        assert!(normalize_phone("12").is_err());
        assert!(normalize_phone("600+111222").is_err());
    }

    #[test]
    fn redacts_secret_from_body() {
        let body = redact("ERROR invalid apikey secret-key", "secret-key");
        assert!(!body.contains("secret-key"));
        assert!(upstream_failed(200, &body));
        assert!(!upstream_failed(200, "Message queued."));
    }
}
