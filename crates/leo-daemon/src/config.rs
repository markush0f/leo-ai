use std::path::PathBuf;

use leo_core::{EngineConfig, SessionConfig};
use leo_vad::VadConfig;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct FileConfig {
    #[serde(default)]
    pub audio: AudioSection,
    #[serde(default)]
    pub vad: VadSection,
    #[serde(default)]
    pub barge_in: BargeSection,
    #[serde(default)]
    pub wake: WakeSection,
    #[serde(default)]
    pub llm: LlmSection,
    #[serde(default)]
    pub stt: SttSection,
}

#[derive(Debug, Deserialize)]
pub struct AudioSection {
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default = "default_sink")]
    pub sink: String,
}

fn default_source() -> String {
    "@DEFAULT_SOURCE@".into()
}
fn default_sink() -> String {
    "@DEFAULT_SINK@".into()
}

impl Default for AudioSection {
    fn default() -> Self {
        Self {
            source: default_source(),
            sink: default_sink(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct VadSection {
    #[serde(default = "default_hangover")]
    pub hangover_ms: u32,
}

fn default_hangover() -> u32 {
    500
}

impl Default for VadSection {
    fn default() -> Self {
        Self {
            hangover_ms: default_hangover(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BargeSection {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_rms")]
    pub rms_threshold: f32,
}

fn default_true() -> bool {
    true
}
fn default_rms() -> f32 {
    0.035
}

impl Default for BargeSection {
    fn default() -> Self {
        Self {
            enabled: true,
            rms_threshold: default_rms(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct WakeSection {
    pub model: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct LlmSection {
    #[serde(default = "default_provider")]
    pub provider: String,
    pub model: Option<String>,
    #[serde(default = "default_system")]
    pub system: String,
}

fn default_provider() -> String {
    "grok".into()
}

fn default_system() -> String {
    "Eres Leo, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta."
        .into()
}

impl Default for LlmSection {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: None,
            system: default_system(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SttSection {
    #[serde(default = "default_stt_lang")]
    pub language: String,
}

fn default_stt_lang() -> String {
    "es".into()
}

impl Default for SttSection {
    fn default() -> Self {
        Self {
            language: default_stt_lang(),
        }
    }
}

impl FileConfig {
    pub fn load() -> Self {
        let path = config_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            tracing::info!(path = %path.display(), "sin config, usando valores por defecto");
            return Self::default();
        };
        match toml::from_str(&text) {
            Ok(cfg) => cfg,
            Err(err) => {
                tracing::warn!(%err, "config inválida, usando valores por defecto");
                Self::default()
            }
        }
    }

    pub fn engine(&self) -> EngineConfig {
        let hangover_frames = (self.vad.hangover_ms / 20).max(1);
        EngineConfig {
            source: self.audio.source.clone(),
            sink: self.audio.sink.clone(),
            session: SessionConfig {
                barge_in: self.barge_in.enabled,
                barge_in_rms: self.barge_in.rms_threshold,
                ..SessionConfig::default()
            },
            vad: VadConfig {
                hangover_frames,
                ..VadConfig::default()
            },
        }
    }

    pub fn wake_model(&self) -> Option<PathBuf> {
        self.wake.model.clone().or_else(|| {
            let p = config_dir().join("wake/leo.rpw");
            p.exists().then_some(p)
        })
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".").join(".config"))
        .join("leo-ai")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn ensure_dirs() {
    let _ = std::fs::create_dir_all(config_dir().join("wake"));
}
