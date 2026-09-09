use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use tokio::runtime::Handle;

use crate::wav::pcm_f32_to_wav;
use crate::{SttEngine, SttError};

const STT_URL: &str = "https://api.x.ai/v1/stt";

#[derive(Deserialize)]
struct SttResponse {
    text: Option<String>,
}

pub struct GrokStt {
    http: reqwest::Client,
    api_key: String,
    language: Option<String>,
    rt: Handle,
}

impl GrokStt {
    pub fn new(api_key: impl Into<String>, rt: Handle, language: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
            language,
            rt,
        }
    }

    async fn transcribe_async(
        &self,
        pcm: &[f32],
        sample_rate: u32,
    ) -> Result<Option<String>, SttError> {
        if pcm.is_empty() {
            return Ok(None);
        }
        let wav = pcm_f32_to_wav(pcm, sample_rate);
        let part = Part::bytes(wav)
            .file_name("speech.wav")
            .mime_str("audio/wav")
            .map_err(|e| SttError::Failed(e.to_string()))?;
        let mut form = Form::new().text("model", "grok-stt");
        if let Some(lang) = &self.language {
            form = form.text("language", lang.clone());
        }
        form = form.part("file", part);

        let response = self
            .http
            .post(STT_URL)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| SttError::Failed(e.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| SttError::Failed(e.to_string()))?;
        if !status.is_success() {
            return Err(SttError::Failed(format!("http {status}: {body}")));
        }
        let parsed: SttResponse =
            serde_json::from_str(&body).map_err(|e| SttError::Failed(e.to_string()))?;
        let text = parsed
            .text
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        Ok(text)
    }
}

impl SttEngine for GrokStt {
    fn transcribe(&self, pcm: &[f32], sample_rate: u32) -> Result<Option<String>, SttError> {
        self.rt.block_on(self.transcribe_async(pcm, sample_rate))
    }
}
