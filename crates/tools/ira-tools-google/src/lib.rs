//! Google Calendar tools for calendar discovery and event management.
//!
//! `ira-tools` registers these operations when a Google access token or API key
//! is configured. The credential's permissions determine which calls succeed.

/// Google API transport and credentials.
pub mod client;
/// Creates a calendar event.
pub mod create_event;
/// Deletes a calendar event.
pub mod delete_event;
mod error;
/// Retrieves an event by ID.
pub mod get_event;
/// Lists calendars available to the client.
pub mod list_calendars;
/// Lists events in a calendar.
pub mod list_events;
/// Updates an existing event.
pub mod update_event;

pub use client::Client;
pub use error::Error;

pub(crate) fn client_enc(s: &str) -> String {
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
