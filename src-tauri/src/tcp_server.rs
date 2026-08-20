// tcp_server.rs
// Async TCP server that mimics a RAW/JetDirect thermal printer on 127.0.0.1:9100.
// Each incoming TCP connection is treated as one print job.
// Raw bytes are read without modification and passed to the ESC/POS parser.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tauri::{AppHandle, Emitter};
use log::{info, warn, error};
use uuid::Uuid;

use crate::escpos_parser::EscPosParser;
use crate::print_job::{JobSource, PrintJob};
use crate::printer_state::PaperWidth;
use crate::renderer::ReceiptRenderer;

// ── Shared server state ───────────────────────────────────────────────────────

pub struct ServerState {
    pub running: bool,
    pub port: u16,
    pub paper_width: PaperWidth,
    pub jobs_received: u32,
    pub last_job: Option<PrintJob>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            running: false,
            port: 9100,
            paper_width: PaperWidth::Mm80,
            jobs_received: 0,
            last_job: None,
        }
    }
}

// ── Tauri events emitted to frontend ─────────────────────────────────────────
//
// "server-status"   → { running: bool, port: u16, address: String }
// "job-received"    → { id, byte_count, hex_dump, html, warnings, commands }
// "server-log"      → { level: "info"|"warn"|"error", message: String }
// "connection-open" → { peer: String }
// "connection-close"→ { peer: String, bytes: usize }

/// Start the TCP server. Returns a shutdown sender so the caller can stop it.
pub async fn run_server(
    app: AppHandle,
    state: Arc<Mutex<ServerState>>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    let (port, paper_width) = {
        let s = state.lock().await;
        (s.port, s.paper_width)
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            let msg = format!("Failed to bind {}:{} — {}", addr.ip(), port, e);
            error!("{}", msg);
            emit_log(&app, "error", &msg);
            let mut s = state.lock().await;
            s.running = false;
            let _ = app.emit("server-status", serde_json::json!({
                "running": false, "port": port, "address": addr.to_string(),
                "error": msg
            }));
            return;
        }
    };

    info!("Virtual printer listening on {}", addr);
    emit_log(&app, "info", &format!("TCP server listening on {}", addr));
    let _ = app.emit("server-status", serde_json::json!({
        "running": true, "port": port, "address": addr.to_string()
    }));

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, peer)) => {
                        info!("Connection from {}", peer);
                        emit_log(&app, "info", &format!("Connection from {}", peer));
                        let _ = app.emit("connection-open", serde_json::json!({ "peer": peer.to_string() }));

                        let app2 = app.clone();
                        let state2 = state.clone();
                        tokio::spawn(async move {
                            handle_connection(stream, peer.to_string(), paper_width, app2, state2).await;
                        });
                    }
                    Err(e) => {
                        warn!("Accept error: {}", e);
                        emit_log(&app, "warn", &format!("Accept error: {}", e));
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!("TCP server shutting down");
                emit_log(&app, "info", "TCP server stopped");
                let _ = app.emit("server-status", serde_json::json!({
                    "running": false, "port": port, "address": addr.to_string()
                }));
                break;
            }
        }
    }
}

// ── Per-connection handler ────────────────────────────────────────────────────

async fn handle_connection(
    mut stream: TcpStream,
    peer: String,
    paper_width: PaperWidth,
    app: AppHandle,
    state: Arc<Mutex<ServerState>>,
) {
    let mut raw_bytes: Vec<u8> = Vec::new();
    let mut buf = [0u8; 4096];

    // Read until connection is closed (EOF) or error
    loop {
        match stream.read(&mut buf).await {
            Ok(0) => break, // EOF — connection closed cleanly
            Ok(n) => {
                raw_bytes.extend_from_slice(&buf[..n]);
            }
            Err(e) => {
                warn!("Read error from {}: {}", peer, e);
                emit_log(&app, "warn", &format!("Read error from {}: {}", peer, e));
                break;
            }
        }
    }

    let byte_count = raw_bytes.len();
    info!("Connection from {} closed. Received {} bytes.", peer, byte_count);
    let _ = app.emit("connection-close", serde_json::json!({ "peer": peer, "bytes": byte_count }));
    emit_log(&app, "info", &format!("Job complete from {} — {} bytes", peer, byte_count));

    if byte_count == 0 {
        emit_log(&app, "warn", &format!("Empty job from {} — ignored", peer));
        return;
    }

    // Parse and render on a blocking thread (CPU-bound work)
    let peer2 = peer.clone();
    let app2 = app.clone();
    let result = tokio::task::spawn_blocking(move || {
        process_job(raw_bytes, paper_width, JobSource::TcpConnection { peer_addr: peer2 })
    })
    .await;

    match result {
        Ok(job) => {
            let html = ReceiptRenderer::render_html(&job, paper_width);
            let payload = build_job_payload(&job, &html);

            // Update shared state
            {
                let mut s = state.lock().await;
                s.jobs_received += 1;
                s.last_job = Some(job);
            }

            let _ = app2.emit("job-received", payload);
        }
        Err(e) => {
            error!("Job processing panicked: {:?}", e);
            emit_log(&app, "error", &format!("Job processing error: {:?}", e));
        }
    }
}

// ── Job processing (blocking, called via spawn_blocking) ──────────────────────

pub fn process_job(raw_bytes: Vec<u8>, paper_width: PaperWidth, source: JobSource) -> PrintJob {
    let job_id = Uuid::new_v4().to_string();
    let mut job = PrintJob::new(job_id, source, raw_bytes);
    let mut parser = EscPosParser::new(paper_width);
    parser.parse(&mut job);
    job
}

// ── Payload builder ───────────────────────────────────────────────────────────

pub fn build_job_payload(job: &PrintJob, html: &str) -> serde_json::Value {
    serde_json::json!({
        "id": job.id,
        "received_at": job.received_at.to_rfc3339(),
        "byte_count": job.byte_count,
        "hex_dump": job.hex_dump_formatted(),
        "hex_raw": job.hex_dump(),
        "warnings": job.warnings,
        "commands": job.parsed_commands.iter().map(|c| serde_json::json!({
            "offset": format!("{:04X}", c.byte_offset),
            "bytes": c.raw_bytes.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "),
            "desc": c.description
        })).collect::<Vec<_>>(),
        "html": html,
        "element_count": 0, // reserved
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn emit_log(app: &AppHandle, level: &str, message: &str) {
    let _ = app.emit("server-log", serde_json::json!({
        "level": level,
        "message": message,
        "ts": chrono::Utc::now().format("%H:%M:%S").to_string()
    }));
}
