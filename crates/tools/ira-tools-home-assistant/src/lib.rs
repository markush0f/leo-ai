//! Home Assistant state queries and service calls.
//!
//! `ira-tools` enables this integration when a server URL and token are
//! available. Reading state and invoking services use the same HTTP client.

/// Invokes a Home Assistant service.
pub mod call_service;
/// Server configuration and authenticated HTTP transport.
pub mod client;
mod error;
/// Retrieves the state of one entity.
pub mod get_state;
/// Lists entity states.
pub mod list_states;

pub use client::Client;
pub use error::Error;
