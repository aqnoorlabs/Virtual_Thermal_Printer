// lib.rs
// Core library entry point.
// Declares all modules and re-exports the Tauri app builder.

pub mod printer_state;
pub mod print_job;
pub mod escpos_parser;
pub mod barcode;
pub mod qrcode;
pub mod renderer;
pub mod tcp_server;
pub mod commands;
pub mod test_receipt;

#[cfg(test)]
mod tests;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::get_server_status,
            commands::send_raw_bytes,
            commands::send_test_receipt,
            commands::send_upi_test_receipt,
            commands::get_last_job_debug,
            commands::clear_preview,
            commands::save_receipt_html,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
