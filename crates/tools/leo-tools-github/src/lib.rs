//! GitHub issue and pull-request tools.
//!
//! The client owns HTTP configuration; operation modules expose model-facing
//! schemas and typed execution. `leo-tools` registers this integration when
//! `GITHUB_TOKEN` or `GH_TOKEN` is configured.

/// Authenticated GitHub HTTP transport.
pub mod client;
/// Creates an issue in a repository.
pub mod create_issue;
/// Opens a pull request between repository branches.
pub mod create_pull_request;
mod error;
/// Retrieves one issue by number.
pub mod get_issue;
/// Lists repository issues.
pub mod list_issues;
/// Lists repository pull requests.
pub mod list_pull_requests;

pub use client::Client;
pub use error::Error;
