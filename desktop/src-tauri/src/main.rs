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
