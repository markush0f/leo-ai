//! Voice conversation orchestration.
//!
//! [`Session`] decides transitions and produces actions without performing I/O.
//! [`spawn_engine`] connects those actions to capture, playback, and the STT,
//! LLM, and TTS traits on a dedicated thread.

mod engine;
mod llm;
mod session;

pub use engine::{EngineConfig, EngineHandle, spawn_engine};
pub use llm::{LlmEngine, NullLlm};
pub use session::{Action, Command, Session, SessionConfig, SessionEvent, State};
