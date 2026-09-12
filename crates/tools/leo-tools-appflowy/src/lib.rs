pub mod client;
pub mod create_page;
pub mod delete_page;
mod error;
pub mod get_page;
pub mod markdown;
pub mod search_pages;
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
