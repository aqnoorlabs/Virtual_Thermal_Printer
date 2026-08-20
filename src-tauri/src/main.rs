// main.rs — Tauri application entry point

// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    aqnoor_virtual_printer_lib::run();
}
