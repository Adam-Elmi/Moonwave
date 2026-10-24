// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
  // WebKitGTK's DMA-BUF renderer lags on some Wayland + AMD setups (Adam's Hyprland, 2026-10-24);
  // the older path is smooth. Only when the user hasn't chosen themselves
  #[cfg(target_os = "linux")]
  if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
    unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
  }
  moonwave_lib::run();
}
