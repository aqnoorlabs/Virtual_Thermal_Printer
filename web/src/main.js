// src/main.js
// Frontend logic for AqNoor Virtual Thermal Printer.
// Calls Tauri commands and handles events from the Rust backend.
// Uses window.__TAURI__ globals (withGlobalTauri: true in tauri.conf.json).

const { invoke } = window.__TAURI__.core;
const { listen }  = window.__TAURI__.event;

// dialog.save is from tauri-plugin-dialog globals (injected when plugin is active)
const dialogSave = window.__TAURI_PLUGIN_DIALOG__?.save ?? null;

// ── State ─────────────────────────────────────────────────────────────────────
let selectedWidth = 80;
let totalJobs = 0;
let totalBytes = 0;
let lastJobId = null;

// ── DOM refs ──────────────────────────────────────────────────────────────────
const $ = id => document.getElementById(id);

const portInput       = $('port-input');
const btnStart        = $('btn-start');
const btnStop         = $('btn-stop');
const btn80mm         = $('btn-80mm');
const btn58mm         = $('btn-58mm');
const btnTestReceipt  = $('btn-test-receipt');
const btnTestUpi      = $('btn-test-upi');
const btnClear        = $('btn-clear');
const btnSaveHtml     = $('btn-save-html');
const btnSendRaw      = $('btn-send-raw');
const rawHexInput     = $('raw-hex-input');

const statusBadge     = $('status-badge');
const statusText      = $('status-text');

const statJobs        = $('stat-jobs');
const statBytes       = $('stat-bytes');
const statPort        = $('stat-port');
const statWidth       = $('stat-width');

const previewPlaceholder = $('preview-placeholder');
const previewFrame    = $('preview-frame');
const previewMeta     = $('preview-meta');

const logOutput       = $('log-output');
const hexOutput       = $('hex-output');
const commandsOutput  = $('commands-output');
const warningsOutput  = $('warnings-output');
const hexByteCount    = $('hex-byte-count');
const cmdCount        = $('cmd-count');
const warnCount       = $('warn-count');

const toastEl         = $('toast');

// ── Tab switching ─────────────────────────────────────────────────────────────
document.querySelectorAll('.debug-tab').forEach(tab => {
  tab.addEventListener('click', () => {
    document.querySelectorAll('.debug-tab').forEach(t => t.classList.remove('active'));
    document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
    tab.classList.add('active');
    $(`tab-${tab.dataset.tab}`).classList.add('active');
  });
});

// ── Paper width toggle ────────────────────────────────────────────────────────
btn80mm.addEventListener('click', () => {
  selectedWidth = 80;
  btn80mm.classList.add('active');
  btn58mm.classList.remove('active');
  statWidth.textContent = '80mm';
});
btn58mm.addEventListener('click', () => {
  selectedWidth = 58;
  btn58mm.classList.add('active');
  btn80mm.classList.remove('active');
  statWidth.textContent = '58mm';
});

// ── Start server ──────────────────────────────────────────────────────────────
btnStart.addEventListener('click', async () => {
  const port = parseInt(portInput.value, 10);
  if (isNaN(port) || port < 1024 || port > 65535) {
    toast('Invalid port number (1024–65535)');
    return;
  }
  btnStart.disabled = true;
  try {
    const msg = await invoke('start_server', {
      port,
      paperWidthMm: selectedWidth,
    });
    toast(msg);
    statPort.textContent = port;
  } catch (err) {
    toast(`Error: ${err}`, true);
    btnStart.disabled = false;
  }
});

// ── Stop server ───────────────────────────────────────────────────────────────
btnStop.addEventListener('click', async () => {
  btnStop.disabled = true;
  try {
    const msg = await invoke('stop_server');
    toast(msg);
  } catch (err) {
    toast(`Error: ${err}`, true);
  }
});

// ── Send test receipt ─────────────────────────────────────────────────────────
btnTestReceipt.addEventListener('click', async () => {
  btnTestReceipt.disabled = true;
  try {
    const payload = await invoke('send_test_receipt');
    handleJobPayload(payload);
    toast('Test receipt sent');
  } catch (err) {
    toast(`Error: ${err}`, true);
  } finally {
    btnTestReceipt.disabled = false;
  }
});

// ── Send UPI QR receipt ───────────────────────────────────────────────────────
btnTestUpi.addEventListener('click', async () => {
  btnTestUpi.disabled = true;
  try {
    const payload = await invoke('send_upi_test_receipt');
    handleJobPayload(payload);
    toast('UPI QR test receipt sent');
  } catch (err) {
    toast(`Error: ${err}`, true);
  } finally {
    btnTestUpi.disabled = false;
  }
});

// ── Clear preview ─────────────────────────────────────────────────────────────
btnClear.addEventListener('click', async () => {
  await invoke('clear_preview').catch(() => {});
  showPlaceholder();
  previewMeta.textContent = 'No job received yet';
  hexOutput.textContent = 'No data received yet';
  commandsOutput.innerHTML = 'No commands parsed yet';
  warningsOutput.innerHTML = 'No warnings';
  hexByteCount.textContent = '0 bytes';
  cmdCount.textContent = '0 commands';
  warnCount.textContent = '0';
  toast('Preview cleared');
});

// ── Save as HTML ──────────────────────────────────────────────────────────────
btnSaveHtml.addEventListener('click', async () => {
  try {
    if (!dialogSave) {
      toast('Save dialog not available', true);
      return;
    }
    const path = await dialogSave({
      defaultPath: 'receipt.html',
      filters: [{ name: 'HTML', extensions: ['html'] }],
    });
    if (!path) return;
    await invoke('save_receipt_html', { path });
    toast(`Saved to ${path}`);
  } catch (err) {
    toast(`Save error: ${err}`, true);
  }
});

// ── Send raw hex bytes ────────────────────────────────────────────────────────
btnSendRaw.addEventListener('click', async () => {
  const hexStr = rawHexInput.value.trim();
  if (!hexStr) { toast('Enter hex bytes first'); return; }

  // Parse hex string like "1B 40 41 0A" → [27, 64, 65, 10]
  const parts = hexStr.split(/[\s,]+/).filter(Boolean);
  const bytes = [];
  for (const p of parts) {
    const n = parseInt(p, 16);
    if (isNaN(n) || n < 0 || n > 255) {
      toast(`Invalid hex byte: "${p}"`, true);
      return;
    }
    bytes.push(n);
  }

  btnSendRaw.disabled = true;
  try {
    const payload = await invoke('send_raw_bytes', { bytes });
    handleJobPayload(payload);
    toast(`Sent ${bytes.length} bytes`);
  } catch (err) {
    toast(`Error: ${err}`, true);
  } finally {
    btnSendRaw.disabled = false;
  }
});

$('btn-clear-log').addEventListener('click', () => {
  logOutput.innerHTML = '';
});

// ── Tauri event listeners ─────────────────────────────────────────────────────

// Server status changed
listen('server-status', ({ payload }) => {
  console.log('[TAURI] server-status:', payload);
  if (payload.running) {
    setStatus(true);
  } else {
    setStatus(false);
    btnStart.disabled = false;
    btnStop.disabled = true;
  }
});

// New print job received (TCP or API)
listen('job-received', ({ payload }) => {
  console.log('[TAURI] job-received:', payload?.id, 'bytes:', payload?.byte_count, 'html_len:', payload?.html?.length);
  handleJobPayload(payload);
});

// Server log entry
listen('server-log', ({ payload }) => {
  appendLog(payload.level, payload.message, payload.ts);
});

// Connection events — these fire when a TCP client (e.g. Windows print spooler) connects
listen('connection-open', ({ payload }) => {
  console.log('[TAURI] connection-open from', payload?.peer);
  appendLog('info', `📥 TCP connection from ${payload.peer}`);
});
listen('connection-close', ({ payload }) => {
  console.log('[TAURI] connection-close from', payload?.peer, 'bytes:', payload?.bytes);
  appendLog('info', `✅ Print job received: ${payload.peer} — ${payload.bytes} bytes`);
});

// ── Handlers ──────────────────────────────────────────────────────────────────

function handleJobPayload(payload) {
  if (!payload) return;

  totalJobs++;
  totalBytes += payload.byte_count || 0;
  lastJobId = payload.id;

  statJobs.textContent = totalJobs;
  statBytes.textContent = formatBytes(totalBytes);

  const ts = payload.received_at
    ? new Date(payload.received_at).toLocaleTimeString()
    : new Date().toLocaleTimeString();

  previewMeta.textContent =
    `Job ${payload.id?.slice(0, 8) || '?'} · ${payload.byte_count} bytes · ${ts}`;

  // Receipt preview
  if (payload.html) {
    showReceipt(payload.html);
  }

  // Hex dump
  if (payload.hex_dump) {
    hexOutput.textContent = payload.hex_dump;
    hexByteCount.textContent = `${payload.byte_count} bytes`;
  }

  // Commands
  if (payload.commands?.length > 0) {
    commandsOutput.innerHTML = payload.commands.map(c =>
      `<div class="cmd-entry">
        <span class="cmd-offset">+${c.offset}</span>
        <span class="cmd-bytes">${c.bytes || '—'}</span>
        <span class="cmd-desc">${escHtml(c.desc)}</span>
      </div>`
    ).join('');
    cmdCount.textContent = `${payload.commands.length} commands`;
  }

  // Warnings
  const warns = payload.warnings || [];
  warnCount.textContent = warns.length;
  if (warns.length > 0) {
    warnCount.classList.add('warn');
    warningsOutput.innerHTML = warns.map(w =>
      `<div class="warn-entry">${escHtml(w)}</div>`
    ).join('');
    // Switch to warnings tab if there are any
    // (do NOT auto-switch so user isn't interrupted)
  } else {
    warnCount.classList.remove('warn');
    warningsOutput.innerHTML = '<span style="color:var(--text-dim);padding:8px;display:block;">No warnings — clean parse ✓</span>';
  }

  appendLog('info', `Job received · ${payload.byte_count} bytes · ${payload.commands?.length || 0} commands · ${warns.length} warnings`);
}

function setStatus(running) {
  if (running) {
    statusBadge.className = 'badge badge-running';
    statusText.textContent = 'Running';
    btnStart.disabled = true;
    btnStop.disabled = false;
  } else {
    statusBadge.className = 'badge badge-stopped';
    statusText.textContent = 'Stopped';
    btnStart.disabled = false;
    btnStop.disabled = true;
  }
}

function showReceipt(html) {
  previewPlaceholder.style.display = 'none';
  previewFrame.style.display = 'block';

  // contentDocument.write() is the most reliable method in Tauri's WebView2.
  // blob: URLs can be silently blocked; srcdoc can fail on full HTML documents.
  try {
    const doc = previewFrame.contentDocument || previewFrame.contentWindow?.document;
    if (doc) {
      doc.open();
      doc.write(html);
      doc.close();
    } else {
      // Fallback: srcdoc
      previewFrame.srcdoc = html;
    }
  } catch (e) {
    console.error('[showReceipt] contentDocument.write failed:', e);
    previewFrame.srcdoc = html;
  }
}

function showPlaceholder() {
  previewPlaceholder.style.display = 'flex';
  previewFrame.style.display = 'none';
  try {
    const doc = previewFrame.contentDocument || previewFrame.contentWindow?.document;
    if (doc) { doc.open(); doc.write(''); doc.close(); }
  } catch (_) {}
  previewFrame.removeAttribute('src');
  previewFrame.removeAttribute('srcdoc');
}

function appendLog(level, message, ts) {
  const time = ts || new Date().toLocaleTimeString();
  const entry = document.createElement('div');
  entry.className = 'log-entry';
  entry.innerHTML = `
    <span class="log-ts">${escHtml(time)}</span>
    <span class="log-level-${level}">${level.toUpperCase()}</span>
    <span class="log-msg">${escHtml(message)}</span>
  `;
  logOutput.appendChild(entry);
  logOutput.scrollTop = logOutput.scrollHeight;
}

// ── Toast ─────────────────────────────────────────────────────────────────────
let toastTimer = null;
function toast(msg, isError = false) {
  toastEl.textContent = msg;
  toastEl.style.borderColor = isError ? 'var(--danger)' : 'var(--border)';
  toastEl.classList.add('show');
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toastEl.classList.remove('show'), 2500);
}

// ── Utilities ─────────────────────────────────────────────────────────────────
function escHtml(str) {
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function formatBytes(n) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MB`;
}

// ── Init ──────────────────────────────────────────────────────────────────────
(async function init() {
  try {
    const status = await invoke('get_server_status');
    setStatus(status.running);
    if (status.running) {
      statPort.textContent = status.port;
      statWidth.textContent = status.paper_width === 58 ? '58mm' : '80mm';
    }
    appendLog('info', 'AqNoor Virtual Thermal Printer ready');
    if (status.running) {
      appendLog('info', `Server listening on 127.0.0.1:${status.port} — configure your printer to this address`);
      if (status.port !== 9100) {
        appendLog('warn', `⚠️ Port is ${status.port} (not default 9100). Make sure your printer TCP port matches!`);
      }
    } else {
      appendLog('info', 'Set port and click ▶ Start Server, then configure your printer to 127.0.0.1:9100');
    }
  } catch (e) {
    appendLog('error', `Init error: ${e}`);
  }
})();
