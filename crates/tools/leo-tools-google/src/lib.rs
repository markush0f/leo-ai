pub mod client;
pub mod create_event;
pub mod delete_event;
mod error;
pub mod get_event;
pub mod list_calendars;
pub mod list_events;
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
