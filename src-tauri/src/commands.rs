// commands.rs
// Tauri command handlers exposed to the frontend via IPC.
// All privileged operations (TCP, parsing, file I/O) stay in Rust.

use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{broadcast, Mutex};
use log::info;

use crate::print_job::JobSource;
use crate::printer_state::PaperWidth;
use crate::renderer::ReceiptRenderer;
use crate::tcp_server::{build_job_payload, process_job, run_server, ServerState};

// ── App state (managed by Tauri) ──────────────────────────────────────────────

pub struct AppState {
    pub server: Arc<Mutex<ServerState>>,
    pub shutdown_tx: Arc<Mutex<Option<broadcast::Sender<()>>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            server: Arc::new(Mutex::new(ServerState::default())),
            shutdown_tx: Arc::new(Mutex::new(None)),
        }
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Start the virtual printer TCP server.
#[tauri::command]
pub async fn start_server(
    port: u16,
    paper_width_mm: u8,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    let mut srv = state.server.lock().await;
    if srv.running {
        return Err("Server is already running".to_string());
    }

    let paper_width = if paper_width_mm == 58 { PaperWidth::Mm58 } else { PaperWidth::Mm80 };
    srv.port = port;
    srv.paper_width = paper_width;
    srv.running = true;
    drop(srv);

    // Create shutdown channel
    let (tx, rx) = broadcast::channel::<()>(1);
    {
        let mut shutdown = state.shutdown_tx.lock().await;
        *shutdown = Some(tx);
    }

    let server_arc = state.server.clone();
    tokio::spawn(async move {
        run_server(app, server_arc, rx).await;
    });

    info!("Server started on port {} ({}mm)", port, paper_width_mm);
    Ok(format!("Server started on 127.0.0.1:{}", port))
}

/// Stop the virtual printer TCP server.
#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>) -> Result<String, String> {
    let mut shutdown = state.shutdown_tx.lock().await;
    if let Some(tx) = shutdown.take() {
        let _ = tx.send(());
        let mut srv = state.server.lock().await;
        srv.running = false;
        Ok("Server stopped".to_string())
    } else {
        Err("Server is not running".to_string())
    }
}

/// Get the current server status.
#[tauri::command]
pub async fn get_server_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let srv = state.server.lock().await;
    Ok(serde_json::json!({
        "running": srv.running,
        "port": srv.port,
        "paper_width": if srv.paper_width == PaperWidth::Mm80 { 80 } else { 58 },
        "jobs_received": srv.jobs_received,
    }))
}

/// Accept raw ESC/POS bytes directly (for POS app integration or testing).
/// `bytes` is an array of u8 integers from the frontend.
#[tauri::command]
pub async fn send_raw_bytes(
    bytes: Vec<u8>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    if bytes.is_empty() {
        return Err("Empty byte array".to_string());
    }

    let paper_width = {
        let srv = state.server.lock().await;
        srv.paper_width
    };

    let source = JobSource::DirectApi;
    let job = tokio::task::spawn_blocking(move || process_job(bytes, paper_width, source))
        .await
        .map_err(|e| format!("Processing error: {:?}", e))?;

    let html = ReceiptRenderer::render_html(&job, paper_width);
    let payload = build_job_payload(&job, &html);

    // Update state
    {
        let mut srv = state.server.lock().await;
        srv.jobs_received += 1;
        srv.last_job = Some(job);
    }

    let _ = app.emit("job-received", &payload);
    Ok(payload)
}

/// Send a pre-built test receipt (AqNoor Pharmacy sample).
#[tauri::command]
pub async fn send_test_receipt(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    let bytes = crate::test_receipt::build_test_receipt();
    let paper_width = {
        let srv = state.server.lock().await;
        srv.paper_width
    };

    let source = JobSource::DirectApi;
    let job = tokio::task::spawn_blocking(move || process_job(bytes, paper_width, source))
        .await
        .map_err(|e| format!("Test receipt error: {:?}", e))?;

    let html = ReceiptRenderer::render_html(&job, paper_width);
    let payload = build_job_payload(&job, &html);

    {
        let mut srv = state.server.lock().await;
        srv.jobs_received += 1;
        srv.last_job = Some(job);
    }

    let _ = app.emit("job-received", &payload);
    Ok(payload)
}

/// Send a sample pharmacy receipt with UPI QR code.
#[tauri::command]
pub async fn send_upi_test_receipt(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    let bytes = crate::test_receipt::build_upi_test_receipt();
    let paper_width = {
        let srv = state.server.lock().await;
        srv.paper_width
    };

    let source = JobSource::DirectApi;
    let job = tokio::task::spawn_blocking(move || process_job(bytes, paper_width, source))
        .await
        .map_err(|e| format!("Test receipt error: {:?}", e))?;

    let html = ReceiptRenderer::render_html(&job, paper_width);
    let payload = build_job_payload(&job, &html);

    {
        let mut srv = state.server.lock().await;
        srv.jobs_received += 1;
        srv.last_job = Some(job);
    }

    let _ = app.emit("job-received", &payload);
    Ok(payload)
}

/// Get the raw hex dump + parsed commands from the last received job.
#[tauri::command]
pub async fn get_last_job_debug(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let srv = state.server.lock().await;
    match &srv.last_job {
        Some(job) => Ok(serde_json::json!({
            "id": job.id,
            "byte_count": job.byte_count,
            "hex_dump": job.hex_dump_formatted(),
            "warnings": job.warnings,
            "commands": job.parsed_commands.iter().map(|c| serde_json::json!({
                "offset": format!("{:04X}", c.byte_offset),
                "bytes": c.raw_bytes.iter().map(|b| format!("{:02X}",b)).collect::<Vec<_>>().join(" "),
                "desc": c.description
            })).collect::<Vec<_>>(),
        })),
        None => Ok(serde_json::json!({ "error": "No jobs received yet" })),
    }
}

/// Clear the current job / receipt preview.
#[tauri::command]
pub async fn clear_preview(state: State<'_, AppState>) -> Result<(), String> {
    let mut srv = state.server.lock().await;
    srv.last_job = None;
    Ok(())
}

/// Save the last rendered receipt as an HTML file (the frontend can also use html2canvas).
#[tauri::command]
pub async fn save_receipt_html(
    path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let srv = state.server.lock().await;
    match &srv.last_job {
        Some(job) => {
            let html = ReceiptRenderer::render_html(job, srv.paper_width);
            std::fs::write(&path, html.as_bytes())
                .map_err(|e| format!("Failed to write file: {}", e))?;
            Ok(format!("Saved to {}", path))
        }
        None => Err("No receipt to save".to_string()),
    }
}
