//! Local process and desktop integration tools.
//!
//! Operations run in the host user's session. Desktop support depends on the
//! available applications and session utilities; failures become tool errors.

/// Application launch, notifications, URLs, and clipboard operations.
pub mod desktop;
mod error;
/// Process inspection and termination.
pub mod process;

pub use error::Error;
