//! AppFlowy page tools and Markdown conversion.
//!
//! The client handles server configuration and authentication; operation
//! modules expose schemas and typed execution for the shared tool registry.

/// AppFlowy configuration, authentication, and HTTP transport.
pub mod client;
/// Creates pages and exposes the write operation's schema.
pub mod create_page;
/// Deletes a page.
pub mod delete_page;
mod error;
/// Retrieves page data.
pub mod get_page;
/// Converts Markdown content for AppFlowy documents.
pub mod markdown;
/// Searches workspace pages.
pub mod search_pages;
/// Updates page content.
pub mod update_page;

pub use client::{Client, Config};
pub use error::Error;

pub(crate) fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
