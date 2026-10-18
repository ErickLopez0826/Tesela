//! Interfaz (lado Rust): comandos Tauri, adaptador de entrada hacia los casos de uso.

#[cfg(windows)]
pub mod bar;
pub mod commands;
pub mod focus_border;
pub mod modules_protocol;
#[cfg(windows)]
pub mod shell;
#[cfg(windows)]
pub mod taskbar;
pub mod tray;
pub mod updates;
pub mod webview;
