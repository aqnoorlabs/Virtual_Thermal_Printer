// print_job.rs
// Typed internal representation of a complete print job.
// The ESC/POS parser produces this; the renderer consumes it.
// No I/O or UI concerns here.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::printer_state::TextStyle;

/// Type of barcode (ESC/POS GS k n)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BarcodeType {
    UpcA,
    UpcE,
    Ean13,
    Ean8,
    Code39,
    Itf,
    Codabar,
    Code93,
    Code128,
    Pdf417,
    QrCode,
    Unknown(u8),
}

impl BarcodeType {
    pub fn from_esc_n(n: u8) -> Self {
        match n {
            // Old format: n = 0–8
            0      => BarcodeType::UpcA,
            1      => BarcodeType::UpcE,
            2      => BarcodeType::Ean13,
            3      => BarcodeType::Ean8,
            4      => BarcodeType::Code39,
            5      => BarcodeType::Itf,
            6      => BarcodeType::Codabar,
            7      => BarcodeType::Code93,
            8      => BarcodeType::Code128,
            10 | 11 | 32 => BarcodeType::QrCode,
            // New / extended format: n = 65–73 (0x41–0x49)
            65     => BarcodeType::UpcA,    // 'A'
            66     => BarcodeType::UpcE,    // 'B'
            67     => BarcodeType::Ean13,   // 'C'
            68     => BarcodeType::Ean8,    // 'D'
            69     => BarcodeType::Code39,  // 'E'
            70     => BarcodeType::Itf,     // 'F'
            71     => BarcodeType::Codabar, // 'G'
            72     => BarcodeType::Code93,  // 'H'
            73     => BarcodeType::Code128, // 'I'
            97 | 104 => BarcodeType::QrCode,
            _      => BarcodeType::Unknown(n),
        }
    }
}

/// HRI (Human Readable Interpretation) position for barcodes
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum HriPosition {
    None,
    Above,
    Below,
    Both,
}

/// QR code error correction level
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum QrErrorCorrection {
    L, // ~7%
    M, // ~15%
    Q, // ~25%
    H, // ~30%
}

/// A single raster image row (from ESC * or GS v 0)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RasterImage {
    pub width_dots: u32,
    pub height_dots: u32,
    /// PNG bytes, base64-encoded for IPC
    pub png_b64: String,
}

/// A single element within a print job.
/// The renderer processes these in order to produce the receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrintElement {
    /// Printable text with associated formatting snapshot
    Text {
        content: String,
        style: TextStyle,
    },

    /// Line feed: advance paper by `lines` lines
    LineFeed {
        lines: u32,
    },

    /// Feed paper by exact dot count
    FeedDots {
        dots: u32,
    },

    /// Barcode element
    Barcode {
        barcode_type: BarcodeType,
        data: String,
        hri: HriPosition,
        height_dots: u8,
        /// Pre-rendered PNG, base64. None if rendering failed.
        png_b64: Option<String>,
    },

    /// QR code element
    QrCode {
        data: String,
        error_correction: QrErrorCorrection,
        module_size: u8,
        /// Pre-rendered PNG, base64. None if rendering failed.
        png_b64: Option<String>,
    },

    /// Raster image (ESC * / GS v 0)
    Image(RasterImage),

    /// Paper cut — partial=false means full cut
    Cut {
        partial: bool,
    },

    /// A horizontal rule (e.g. dashes printed as text) — detected by renderer
    Rule,
}

/// Debug record of a single parsed ESC/POS command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedCommand {
    pub byte_offset: usize,
    pub raw_bytes: Vec<u8>,
    pub description: String,
}

/// A complete print job produced from one TCP connection or one `send_raw_bytes` call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintJob {
    pub id: String,
    pub received_at: DateTime<Utc>,
    pub source: JobSource,

    /// Ordered elements to render
    pub elements: Vec<PrintElement>,

    /// Raw bytes as received (for debug hex dump)
    pub raw_bytes: Vec<u8>,

    /// All parsed commands (for debug panel)
    pub parsed_commands: Vec<ParsedCommand>,

    /// Parse warnings / unsupported commands
    pub warnings: Vec<String>,

    /// Total byte count
    pub byte_count: usize,
}

/// Where did this job come from?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobSource {
    TcpConnection { peer_addr: String },
    DirectApi,
}

impl PrintJob {
    pub fn new(id: String, source: JobSource, raw_bytes: Vec<u8>) -> Self {
        let byte_count = raw_bytes.len();
        Self {
            id,
            received_at: Utc::now(),
            source,
            elements: Vec::new(),
            raw_bytes,
            parsed_commands: Vec::new(),
            warnings: Vec::new(),
            byte_count,
        }
    }

    /// Hex dump of raw bytes, formatted as "XX XX XX ..."
    pub fn hex_dump(&self) -> String {
        self.raw_bytes
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Hex dump with offsets, 16 bytes per line
    pub fn hex_dump_formatted(&self) -> String {
        let mut out = String::new();
        for (i, chunk) in self.raw_bytes.chunks(16).enumerate() {
            let hex: Vec<String> = chunk.iter().map(|b| format!("{:02X}", b)).collect();
            let ascii: String = chunk
                .iter()
                .map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { '.' })
                .collect();
            out.push_str(&format!(
                "{:04X}  {:<47}  |{}|\n",
                i * 16,
                hex.join(" "),
                ascii
            ));
        }
        out
    }
}
