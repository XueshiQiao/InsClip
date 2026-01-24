#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri_plugin_clipboard_manager::ClipboardExt;
use winpaste::run_app;

fn main() {
    run_app();
}
