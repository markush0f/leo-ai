//! Desktop window binary (`leo-desktop`).
//!
//! Thin OS entry: apply the Linux WebKit workaround, then hand off to
//! [`leo_desktop_lib::run`]. The library crate owns Tauri commands, Postgres,
//! and tools. React in `desktop/src/` is the UI; this process is the native
//! host. Voice control is deferred.
//!
//! # Workspace crates (used from `leo_desktop_lib`)
//!
//! - [`leo_store`] — shared PostgreSQL catalog and local conversations.
//!   `connect` / `migrate` run at setup; `apply_secrets_to_env` exports stored
//!   keys. Commands load/apply [`leo_store::Snapshot`] / [`leo_store::DbOp`],
//!   list and open chats, and persist turns with `append_message` /
//!   `context_messages`. Frontend DTOs expose credential *status* (`db` /
//!   `env` / `falta` / `none`), never the key material.
//! - [`leo_llm`] — `load_dotenv` and [`leo_llm::Client`] from
//!   `Snapshot::client`. [`leo_llm::ChatRequest`] carries history and, for
//!   Grok, `reasoning_effort` from the thinking setting. Tool calls are
//!   transported, not executed, here.
//! - [`leo_tools`] — [`leo_tools::Registry::from_env`] at startup.
//!   [`leo_tools::chat`] runs the tool loop when tools are enabled; otherwise
//!   the turn uses an empty registry. Same integrations as the TUI.
//!
//! Browser `npm run web` mocks this bridge; it does not exercise Postgres or
//! providers.
//!
//! # Linux WebKit
//!
//! NVIDIA + Wayland + WebKitGTK can die with Wayland protocol error 71.
//! If `WEBKIT_DISABLE_DMABUF_RENDERER` is unset, it is set to `1` before GTK
//! starts. A Wayland session with no `GDK_BACKEND` is forced to `x11`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    linux_webkit_workaround();
    leo_desktop_lib::run()
}

/// NVIDIA + Wayland + WebKitGTK dies with
/// `Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display`.
fn linux_webkit_workaround() {
    #[cfg(target_os = "linux")]
    {
        // set_var is unsafe in edition 2024; this runs before GTK/threads.
        unsafe {
            if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            }
            if std::env::var_os("WAYLAND_DISPLAY").is_some()
                && std::env::var_os("GDK_BACKEND").is_none()
            {
                std::env::set_var("GDK_BACKEND", "x11");
            }
        }
    }
}
