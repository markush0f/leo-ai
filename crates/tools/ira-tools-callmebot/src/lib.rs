//! WhatsApp text to the number activated in CallMeBot.
//!
//! `ira-tools` enables this when `CALLMEBOT_PHONE` and `CALLMEBOT_APIKEY` are
//! set. The free API only delivers to that number and does not accept replies.

/// CallMeBot configuration and HTTP transport.
pub mod client;
mod error;
/// Sends one WhatsApp text.
pub mod send_message;

pub use client::Client;
pub use error::Error;
