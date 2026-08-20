// escpos_parser.rs
// Full ESC/POS byte-level state machine parser.
// Processes raw bytes exactly as a real thermal printer would.
// Produces a PrintJob containing typed PrintElement items.
// Never panics on unknown commands — logs and continues.

use crate::barcode::render_barcode;
use crate::print_job::{
    BarcodeType, HriPosition, ParsedCommand, PrintElement, PrintJob, QrErrorCorrection, RasterImage,
};
use crate::printer_state::{Alignment, CodePage, Font, PaperWidth, PrinterState};
use crate::qrcode::render_qrcode;
use log::{debug, warn};

// ── ESC/POS Control Characters ────────────────────────────────────────────────
const ESC: u8 = 0x1B;
const GS: u8 = 0x1D;
const LF: u8 = 0x0A;
const CR: u8 = 0x0D;
const HT: u8 = 0x09;
const FF: u8 = 0x0C;
const NUL: u8 = 0x00;
const CAN: u8 = 0x18;

// ── State Machine ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum ParserState {
    Normal,

    // ESC pending — waiting for command byte
    EscPending,

    // GS pending — waiting for command byte
    GsPending,

    // Single-byte param states (ESC)
    EscAlign,
    EscBold,
    EscUnderline,
    EscPrintMode,
    EscFont,
    EscLineSpacing,
    EscFeedLines,
    EscFeedDots,
    EscCodePage,
    EscUpsideDown,
    EscSkipOne,        // consume one byte and return to Normal

    // Two-byte skip
    EscSkipTwo { got: u8 },

    // ESC * bit image
    EscBitImageMode,
    EscBitImageWidthL { mode: u8 },
    EscBitImageWidthH { mode: u8, wl: u8 },
    EscBitImageData { mode: u8, width: u16, height: u16, remaining: usize, data: Vec<u8> },

    // Single-byte param states (GS)
    GsCharSize,
    GsInverse,
    GsCutType,
    GsCutFeedParam,
    GsBarcodeHeight,
    GsBarcodeWidth,
    GsHriPosition,
    GsBarcodeFont,

    // GS k — barcode
    GsBarcodeTypeSelect,
    GsBarcodeNulTerm { btype: BarcodeType, buf: Vec<u8> },
    GsBarcodeNewLen { btype: BarcodeType },
    GsBarcodeNewData { btype: BarcodeType, remaining: usize, buf: Vec<u8> },

    // ESC Z — Chinese/POS-58/80 QR format: ESC Z <v> <r> <k> <xL> <xH> <data...>
    EscZVersion,
    EscZEcc { v: u8 },
    EscZSize { v: u8, ec: QrErrorCorrection },
    EscZLenL { v: u8, ec: QrErrorCorrection, size: u8 },
    EscZLenH { v: u8, ec: QrErrorCorrection, size: u8, xl: u8 },
    EscZData { ec: QrErrorCorrection, size: u8, remaining: usize, buf: Vec<u8> },

    // GS ( k — QR code (5-field protocol)
    // GS ( <type=k/8/0> pL pH cn fn [data...]
    GsExtFnType,
    GsExtFnPL,
    GsExtFnPH { pl: u8 },
    GsExtFnCN { pl: u8, ph: u8 },
    GsExtFnFN { pl: u8, ph: u8, cn: u8 },
    GsQrModelN1 { remaining: usize },
    GsQrModelN2 { remaining: usize },
    GsQrSize { remaining: usize },
    GsQrEc { remaining: usize },
    GsQrStore { remaining: usize, buf: Vec<u8> },
    GsQrPrint { remaining: usize },
    GsQrSkip { remaining: usize },

    // GS v 0 — raster image
    GsRasterExpectZero,
    GsRasterMode,
    GsRasterXL { mode: u8 },
    GsRasterXH { mode: u8, xl: u8 },
    GsRasterYL { mode: u8, xl: u8, xh: u8 },
    GsRasterYH { mode: u8, xl: u8, xh: u8, yl: u8 },
    GsRasterData { width_bytes: u32, height: u32, remaining: usize, buf: Vec<u8> },
}

// ── Parser ────────────────────────────────────────────────────────────────────

pub struct EscPosParser {
    state: ParserState,
    pub printer: PrinterState,

    // Current text accumulation (flushed on LF or formatting change)
    line_buf: String,

    // Barcode config (GS h/w/H/f before GS k)
    bc_height: u8,
    bc_width: u8,
    bc_hri: HriPosition,

    // QR code config (GS ( k sub-commands)
    qr_model: u8,
    qr_size: u8,
    qr_ec: QrErrorCorrection,
    qr_data: Vec<u8>,

    // Debug: current byte offset
    offset: usize,
}

impl EscPosParser {
    pub fn new(paper_width: PaperWidth) -> Self {
        let mut printer = PrinterState::default();
        printer.paper_width = paper_width;
        Self {
            state: ParserState::Normal,
            printer,
            line_buf: String::new(),
            bc_height: 162,
            bc_width: 3,
            bc_hri: HriPosition::Below,
            qr_model: 2,
            qr_size: 4,
            qr_ec: QrErrorCorrection::M,
            qr_data: Vec::new(),
            offset: 0,
        }
    }

    /// Parse all bytes in `job.raw_bytes` and populate `job.elements`.
    pub fn parse(&mut self, job: &mut PrintJob) {
        let bytes = job.raw_bytes.clone();
        for &b in &bytes {
            self.step(b, job);
            self.offset += 1;
        }
        // Flush any trailing text
        self.flush_text(job);
    }

    // ── Core step ─────────────────────────────────────────────────────────────

    fn step(&mut self, b: u8, job: &mut PrintJob) {
        let next = match self.state.clone() {
            ParserState::Normal                          => self.on_normal(b, job),
            ParserState::EscPending                      => self.on_esc(b, job),
            ParserState::GsPending                       => self.on_gs(b, job),

            // ESC single-param
            ParserState::EscAlign                        => self.on_esc_align(b, job),
            ParserState::EscBold                         => self.on_esc_bold(b, job),
            ParserState::EscUnderline                    => self.on_esc_underline(b, job),
            ParserState::EscPrintMode                    => self.on_esc_print_mode(b, job),
            ParserState::EscFont                         => self.on_esc_font(b, job),
            ParserState::EscLineSpacing                  => self.on_esc_linespacing(b, job),
            ParserState::EscFeedLines                    => self.on_esc_feed_lines(b, job),
            ParserState::EscFeedDots                     => self.on_esc_feed_dots(b, job),
            ParserState::EscCodePage                     => self.on_esc_codepage(b, job),
            ParserState::EscUpsideDown                   => { self.printer.upside_down = b != 0; ParserState::Normal }
            ParserState::EscSkipOne                      => ParserState::Normal,
            ParserState::EscSkipTwo { got }              => {
                if got == 0 { ParserState::EscSkipTwo { got: 1 } } else { ParserState::Normal }
            }

            // ESC * bit image
            ParserState::EscBitImageMode                 => ParserState::EscBitImageWidthL { mode: b },
            ParserState::EscBitImageWidthL { mode }      => ParserState::EscBitImageWidthH { mode, wl: b },
            ParserState::EscBitImageWidthH { mode, wl }  => {
                let width = (wl as u16) | ((b as u16) << 8);
                let bpc: u16 = if mode == 32 || mode == 33 { 3 } else { 1 };
                let height: u16 = bpc * 8;
                let remaining = (width as usize) * (bpc as usize);
                if remaining == 0 { ParserState::Normal }
                else { ParserState::EscBitImageData { mode, width, height, remaining, data: Vec::new() } }
            }
            ParserState::EscBitImageData { mode, width, height, remaining, mut data } => {
                data.push(b);
                if data.len() >= remaining {
                    let wb = (width as u32 + 7) / 8;
                    self.emit_raster(wb, height as u32, data, job);
                    ParserState::Normal
                } else {
                    ParserState::EscBitImageData { mode, width, height, remaining, data }
                }
            }

            // GS single-param
            ParserState::GsCharSize                      => self.on_gs_char_size(b, job),
            ParserState::GsInverse                       => self.on_gs_inverse(b, job),
            ParserState::GsCutType                       => self.on_gs_cut_type(b, job),
            ParserState::GsCutFeedParam                  => {
                self.log_cmd(job, &[GS, b'V', b], "GS V cut feed param (skipped)");
                ParserState::Normal
            }
            ParserState::GsBarcodeHeight                 => {
                self.bc_height = b;
                self.log_cmd(job, &[GS, b'h', b], &format!("GS h height={}", b));
                ParserState::Normal
            }
            ParserState::GsBarcodeWidth                  => {
                self.bc_width = b;
                self.log_cmd(job, &[GS, b'w', b], &format!("GS w width={}", b));
                ParserState::Normal
            }
            ParserState::GsHriPosition                   => {
                self.bc_hri = match b { 0 => HriPosition::None, 1 => HriPosition::Above, 2 => HriPosition::Below, 3 => HriPosition::Both, _ => HriPosition::Below };
                self.log_cmd(job, &[GS, b'H', b], &format!("GS H hri={}", b));
                ParserState::Normal
            }
            ParserState::GsBarcodeFont                   => {
                self.log_cmd(job, &[GS, b'f', b], "GS f barcode font");
                ParserState::Normal
            }

            // GS k barcode
            ParserState::GsBarcodeTypeSelect             => self.on_gs_k_type(b, job),
            ParserState::GsBarcodeNulTerm { btype, mut buf } => {
                if b == NUL {
                    self.emit_barcode(btype, buf, job);
                    ParserState::Normal
                } else {
                    buf.push(b);
                    ParserState::GsBarcodeNulTerm { btype, buf }
                }
            }
            ParserState::GsBarcodeNewLen { btype }       => ParserState::GsBarcodeNewData { btype, remaining: b as usize, buf: Vec::new() },
            ParserState::GsBarcodeNewData { btype, remaining, mut buf } => {
                buf.push(b);
                if buf.len() >= remaining {
                    self.emit_barcode(btype, buf, job);
                    ParserState::Normal
                } else {
                    ParserState::GsBarcodeNewData { btype, remaining, buf }
                }
            }

            // ESC Z QR
            ParserState::EscZVersion                     => ParserState::EscZEcc { v: b },
            ParserState::EscZEcc { v }                   => {
                let ec = match b {
                    b'L' | b'l' | 0 | 48 => QrErrorCorrection::L,
                    b'M' | b'm' | 1 | 49 => QrErrorCorrection::M,
                    b'Q' | b'q' | 2 | 50 => QrErrorCorrection::Q,
                    b'H' | b'h' | 3 | 51 => QrErrorCorrection::H,
                    _ => QrErrorCorrection::M,
                };
                ParserState::EscZSize { v, ec }
            }
            ParserState::EscZSize { v, ec }              => {
                let size = b.max(1).min(16);
                ParserState::EscZLenL { v, ec, size }
            }
            ParserState::EscZLenL { v, ec, size }        => ParserState::EscZLenH { v, ec, size, xl: b },
            ParserState::EscZLenH { v: _, ec, size, xl } => {
                let len = (xl as usize) | ((b as usize) << 8);
                if len == 0 {
                    ParserState::Normal
                } else {
                    ParserState::EscZData { ec, size, remaining: len, buf: Vec::new() }
                }
            }
            ParserState::EscZData { ec, size, remaining, mut buf } => {
                buf.push(b);
                if buf.len() >= remaining {
                    self.flush_text(job);
                    let data = String::from_utf8_lossy(&buf).to_string();
                    let png_b64 = render_qrcode(&data, ec, size)
                        .map_err(|e| self.warn(job, &format!("QR render failed: {}", e)))
                        .ok();
                    self.log_cmd(job, &[], &format!("ESC Z QR code data={:?}", data));
                    job.elements.push(PrintElement::QrCode {
                        data,
                        error_correction: ec,
                        module_size: size,
                        png_b64,
                    });
                    ParserState::Normal
                } else {
                    ParserState::EscZData { ec, size, remaining, buf }
                }
            }

            // GS ( k QR
            ParserState::GsExtFnType                     => ParserState::GsExtFnPL,
            ParserState::GsExtFnPL                       => ParserState::GsExtFnPH { pl: b },
            ParserState::GsExtFnPH { pl }                => ParserState::GsExtFnCN { pl, ph: b },
            ParserState::GsExtFnCN { pl, ph }            => ParserState::GsExtFnFN { pl, ph, cn: b },
            ParserState::GsExtFnFN { pl, ph, cn }        => self.on_gs_qr_fn(b, pl, ph, cn, job),

            ParserState::GsQrModelN1 { remaining }       => {
                self.qr_model = b;
                if remaining > 1 {
                    ParserState::GsQrModelN2 { remaining: remaining - 1 }
                } else {
                    ParserState::Normal
                }
            }
            ParserState::GsQrModelN2 { remaining }       => {
                if remaining > 1 {
                    ParserState::GsQrSkip { remaining: remaining - 1 }
                } else {
                    ParserState::Normal
                }
            }

            ParserState::GsQrSize { remaining }          => {
                self.qr_size = b.max(1).min(16);
                if remaining > 1 {
                    ParserState::GsQrSkip { remaining: remaining - 1 }
                } else {
                    ParserState::Normal
                }
            }

            ParserState::GsQrEc { remaining }            => {
                self.qr_ec = match b {
                    48 | 0 => QrErrorCorrection::L,
                    49 | 1 => QrErrorCorrection::M,
                    50 | 2 => QrErrorCorrection::Q,
                    51 | 3 => QrErrorCorrection::H,
                    _ => QrErrorCorrection::M,
                };
                if remaining > 1 {
                    ParserState::GsQrSkip { remaining: remaining - 1 }
                } else {
                    ParserState::Normal
                }
            }

            ParserState::GsQrStore { remaining, mut buf } => {
                buf.push(b);
                if buf.len() >= remaining {
                    if !buf.is_empty() && (buf[0] == 0 || buf[0] == b'0') && buf.len() > 1 {
                        self.qr_data = buf[1..].to_vec();
                    } else {
                        self.qr_data = buf;
                    }
                    ParserState::Normal
                } else {
                    ParserState::GsQrStore { remaining, buf }
                }
            }

            ParserState::GsQrPrint { remaining }         => {
                if remaining > 1 {
                    ParserState::GsQrPrint { remaining: remaining - 1 }
                } else {
                    ParserState::Normal
                }
            }

            ParserState::GsQrSkip { remaining }          => {
                if remaining > 1 {
                    ParserState::GsQrSkip { remaining: remaining - 1 }
                } else {
                    ParserState::Normal
                }
            }

            // GS v 0 raster
            ParserState::GsRasterExpectZero              => {
                if b == b'0' || b == 0 {
                    ParserState::GsRasterMode
                } else {
                    // Fallback: byte is mode directly
                    ParserState::GsRasterXL { mode: b }
                }
            }
            ParserState::GsRasterMode                    => ParserState::GsRasterXL { mode: b },
            ParserState::GsRasterXL { mode }             => ParserState::GsRasterXH { mode, xl: b },
            ParserState::GsRasterXH { mode, xl }         => ParserState::GsRasterYL { mode, xl, xh: b },
            ParserState::GsRasterYL { mode, xl, xh }     => ParserState::GsRasterYH { mode, xl, xh, yl: b },
            ParserState::GsRasterYH { mode: _, xl, xh, yl } => {
                let width_bytes = (xl as u32) | ((xh as u32) << 8);
                let height      = (yl as u32) | ((b  as u32) << 8);
                let remaining   = (width_bytes * height) as usize;
                if remaining == 0 { ParserState::Normal }
                else { ParserState::GsRasterData { width_bytes, height, remaining, buf: Vec::new() } }
            }
            ParserState::GsRasterData { width_bytes, height, remaining, mut buf } => {
                buf.push(b);
                if buf.len() >= remaining {
                    self.emit_raster(width_bytes, height, buf, job);
                    ParserState::Normal
                } else {
                    ParserState::GsRasterData { width_bytes, height, remaining, buf }
                }
            }
        };
        self.state = next;
    }

    // ── Normal mode ───────────────────────────────────────────────────────────

    fn on_normal(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        match b {
            ESC  => ParserState::EscPending,
            GS   => ParserState::GsPending,

            LF   => {
                self.flush_text(job);
                job.elements.push(PrintElement::LineFeed { lines: 1 });
                self.log_cmd(job, &[LF], "LF");
                ParserState::Normal
            }
            CR   => {
                // CR alone does not feed on most ESC/POS — ignore
                self.log_cmd(job, &[CR], "CR (ignored)");
                ParserState::Normal
            }
            HT   => {
                let col = self.line_buf.len() % 8;
                let pad = 8 - col;
                self.line_buf.push_str(&" ".repeat(pad));
                ParserState::Normal
            }
            FF   => {
                self.flush_text(job);
                job.elements.push(PrintElement::Cut { partial: false });
                self.log_cmd(job, &[FF], "FF (form feed / cut)");
                ParserState::Normal
            }
            NUL | CAN => ParserState::Normal,

            0x20..=0x7E => {
                self.line_buf.push(b as char);
                ParserState::Normal
            }
            0x80..=0xFF => {
                self.line_buf.push(self.decode_high(b));
                ParserState::Normal
            }
            _ => {
                self.warn(job, &format!("Unhandled byte 0x{:02X}", b));
                ParserState::Normal
            }
        }
    }

    // ── ESC dispatch ─────────────────────────────────────────────────────────

    fn on_esc(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        match b {
            b'@' => {
                self.flush_text(job);
                self.printer.reset();
                self.log_cmd(job, &[ESC, b'@'], "ESC @ initialize");
                ParserState::Normal
            }
            b'a' => ParserState::EscAlign,
            b'E' | b'G' => ParserState::EscBold,
            b'-' => ParserState::EscUnderline,
            b'!' => ParserState::EscPrintMode,
            b'M' => ParserState::EscFont,
            b'2' => {
                self.printer.line_spacing = 30;
                self.log_cmd(job, &[ESC, b'2'], "ESC 2 default line spacing");
                ParserState::Normal
            }
            b'3' => ParserState::EscLineSpacing,
            b'd' => ParserState::EscFeedLines,
            b'J' => ParserState::EscFeedDots,
            b't' => ParserState::EscCodePage,
            b'{' => ParserState::EscUpsideDown,
            b'V' => ParserState::EscSkipOne, // rotate 90 — consume param
            b'p' => ParserState::EscSkipTwo { got: 0 }, // cash drawer — 2 params
            b'*' => ParserState::EscBitImageMode,
            b'Z' => ParserState::EscZVersion,
            b'i' => {
                // ESC i — immediate full cut (no param on most firmwares)
                self.flush_text(job);
                job.elements.push(PrintElement::Cut { partial: false });
                self.log_cmd(job, &[ESC, b'i'], "ESC i full cut");
                ParserState::Normal
            }
            b'm' => {
                self.flush_text(job);
                job.elements.push(PrintElement::Cut { partial: true });
                self.log_cmd(job, &[ESC, b'm'], "ESC m partial cut");
                ParserState::Normal
            }
            // Single-byte skip commands
            b'r' | b'=' | b'?' | b'K' | b'u' => ParserState::EscSkipOne,
            b'c' => ParserState::EscSkipTwo { got: 0 },
            _ => {
                self.warn(job, &format!("Unknown ESC 0x{:02X} ('{}')", b, b as char));
                ParserState::Normal
            }
        }
    }

    // ── GS dispatch ───────────────────────────────────────────────────────────

    fn on_gs(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        match b {
            b'!' => ParserState::GsCharSize,
            b'B' => ParserState::GsInverse,
            b'V' => ParserState::GsCutType,
            b'h' => ParserState::GsBarcodeHeight,
            b'w' => ParserState::GsBarcodeWidth,
            b'H' => ParserState::GsHriPosition,
            b'f' => ParserState::GsBarcodeFont,
            b'k' => ParserState::GsBarcodeTypeSelect,
            b'(' => ParserState::GsExtFnType,
            b'v' => ParserState::GsRasterExpectZero,
            // 2-byte skip
            b'L' | b'P' => ParserState::EscSkipTwo { got: 0 },
            // 1-byte skip
            b'a' | b'r' | b'I' | b'e' => ParserState::EscSkipOne,
            _ => {
                self.warn(job, &format!("Unknown GS 0x{:02X} ('{}')", b, b as char));
                ParserState::Normal
            }
        }
    }

    // ── ESC param handlers ────────────────────────────────────────────────────

    fn on_esc_align(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        self.flush_text(job);
        self.printer.alignment = match b {
            0 | 48 => Alignment::Left,
            1 | 49 => Alignment::Center,
            2 | 50 => Alignment::Right,
            _ => Alignment::Left,
        };
        self.log_cmd(job, &[ESC, b'a', b], &format!("ESC a {:?}", self.printer.alignment));
        ParserState::Normal
    }

    fn on_esc_bold(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        self.printer.bold = b != 0;
        self.log_cmd(job, &[ESC, b'E', b], &format!("ESC E bold={}", b != 0));
        ParserState::Normal
    }

    fn on_esc_underline(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        self.printer.underline = b.min(2);
        self.log_cmd(job, &[ESC, b'-', b], &format!("ESC - underline={}", b));
        ParserState::Normal
    }

    fn on_esc_print_mode(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        // bit 0: font B; bit 3: bold; bit 4: dbl-height; bit 5: dbl-width; bit 7: underline
        self.printer.font = if b & 0x01 != 0 { Font::B } else { Font::A };
        self.printer.bold = b & 0x08 != 0;
        self.printer.char_height_multiplier = if b & 0x10 != 0 { 2 } else { 1 };
        self.printer.char_width_multiplier  = if b & 0x20 != 0 { 2 } else { 1 };
        self.printer.underline = if b & 0x80 != 0 { 1 } else { 0 };
        self.log_cmd(job, &[ESC, b'!', b], &format!("ESC ! mode=0x{:02X}", b));
        ParserState::Normal
    }

    fn on_esc_font(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        self.printer.font = if b == 1 || b == 49 { Font::B } else { Font::A };
        self.log_cmd(job, &[ESC, b'M', b], &format!("ESC M font={:?}", self.printer.font));
        ParserState::Normal
    }

    fn on_esc_linespacing(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        self.printer.line_spacing = b;
        self.log_cmd(job, &[ESC, b'3', b], &format!("ESC 3 spacing={}", b));
        ParserState::Normal
    }

    fn on_esc_feed_lines(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        self.flush_text(job);
        if b > 0 {
            job.elements.push(PrintElement::LineFeed { lines: b as u32 });
        }
        self.log_cmd(job, &[ESC, b'd', b], &format!("ESC d feed {} lines", b));
        ParserState::Normal
    }

    fn on_esc_feed_dots(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        job.elements.push(PrintElement::FeedDots { dots: b as u32 });
        self.log_cmd(job, &[ESC, b'J', b], &format!("ESC J feed {} dots", b));
        ParserState::Normal
    }

    fn on_esc_codepage(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        self.printer.code_page = match b {
            0  => CodePage::Pc437,
            2  => CodePage::Pc850,
            16 | 17 => CodePage::Windows1252,
            255 => CodePage::Utf8,
            _  => CodePage::Unknown(b),
        };
        self.log_cmd(job, &[ESC, b't', b], &format!("ESC t codepage={}", b));
        ParserState::Normal
    }

    // ── GS param handlers ─────────────────────────────────────────────────────

    fn on_gs_char_size(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        self.printer.char_width_multiplier  = ((b >> 4) & 0x07) + 1;
        self.printer.char_height_multiplier = (b & 0x07) + 1;
        self.log_cmd(job, &[GS, b'!', b], &format!("GS ! size w={}x h={}x", self.printer.char_width_multiplier, self.printer.char_height_multiplier));
        ParserState::Normal
    }

    fn on_gs_inverse(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        self.printer.inverse = b != 0;
        self.log_cmd(job, &[GS, b'B', b], &format!("GS B inverse={}", b != 0));
        ParserState::Normal
    }

    fn on_gs_cut_type(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        self.flush_text(job);
        let (partial, has_feed_param) = match b {
            0 | 48 => (false, false),
            1 | 49 => (true,  false),
            65      => (false, true),
            66      => (true,  true),
            97      => (false, true),
            98      => (true,  true),
            _ => {
                self.warn(job, &format!("GS V unknown type 0x{:02X}", b));
                return ParserState::Normal;
            }
        };
        job.elements.push(PrintElement::Cut { partial });
        self.log_cmd(job, &[GS, b'V', b], &format!("GS V cut partial={}", partial));
        if has_feed_param { ParserState::GsCutFeedParam } else { ParserState::Normal }
    }

    // ── GS k barcode ─────────────────────────────────────────────────────────

    fn on_gs_k_type(&mut self, b: u8, job: &mut PrintJob) -> ParserState {
        match b {
            0..=8 | 10 | 11 | 32 => ParserState::GsBarcodeNulTerm { btype: BarcodeType::from_esc_n(b), buf: Vec::new() },
            65..=73 | 97 | 104 => ParserState::GsBarcodeNewLen { btype: BarcodeType::from_esc_n(b) },
            _ => {
                self.warn(job, &format!("GS k unknown barcode type 0x{:02X}", b));
                ParserState::Normal
            }
        }
    }

    // ── GS ( k QR dispatch ────────────────────────────────────────────────────

    fn on_gs_qr_fn(&mut self, fn_byte: u8, pl: u8, ph: u8, _cn: u8, job: &mut PrintJob) -> ParserState {
        // data_len = pL + pH*256, but already consumed cn and fn (2 bytes)
        // actual remaining data = pL + pH*256 - 2
        let total = (pl as usize) | ((ph as usize) << 8);
        let data_remaining = total.saturating_sub(2);

        match fn_byte {
            65 => {
                // Model — next param bytes (expect n1, n2)
                self.log_cmd(job, &[], "GS ( k QR model");
                if data_remaining > 0 { ParserState::GsQrModelN1 { remaining: data_remaining } }
                else { ParserState::Normal }
            }
            67 => {
                // Module size
                self.log_cmd(job, &[], "GS ( k QR size");
                if data_remaining > 0 { ParserState::GsQrSize { remaining: data_remaining } }
                else { ParserState::Normal }
            }
            69 => {
                // Error correction
                self.log_cmd(job, &[], "GS ( k QR error correction");
                if data_remaining > 0 { ParserState::GsQrEc { remaining: data_remaining } }
                else { ParserState::Normal }
            }
            80 => {
                // Store data
                self.log_cmd(job, &[], "GS ( k QR store data");
                if data_remaining > 0 { ParserState::GsQrStore { remaining: data_remaining, buf: Vec::new() } }
                else { ParserState::Normal }
            }
            81 => {
                // Print stored QR
                self.flush_text(job);
                self.emit_qr(job);
                self.log_cmd(job, &[], "GS ( k QR print");
                if data_remaining > 0 { ParserState::GsQrPrint { remaining: data_remaining } }
                else { ParserState::Normal }
            }
            _ => {
                self.warn(job, &format!("GS ( k unknown fn=0x{:02X}", fn_byte));
                // skip data_remaining bytes
                if data_remaining > 0 {
                    ParserState::GsQrSkip { remaining: data_remaining }
                } else {
                    ParserState::Normal
                }
            }
        }
    }

    // ── Emit helpers ──────────────────────────────────────────────────────────

    /// Flush accumulated line buffer as a Text element
    pub fn flush_text(&mut self, job: &mut PrintJob) {
        if !self.line_buf.is_empty() {
            let content = std::mem::take(&mut self.line_buf);
            self.process_text_content(content, job);
        }
    }

    fn process_text_content(&mut self, content: String, job: &mut PrintJob) {
        let upi_tag = "[UPI_QR:";
        let qr_tag = "[QR:";
        let qrcode_tag = "[QR_CODE:";

        if !content.contains(upi_tag) && !content.contains(qr_tag) && !content.contains(qrcode_tag) {
            let style = self.printer.current_style();
            job.elements.push(PrintElement::Text { content, style });
            return;
        }

        // Process line by line
        for line in content.split('\n') {
            let trimmed = line.trim();
            if trimmed.starts_with(upi_tag) && trimmed.ends_with(']') {
                let uri = &trimmed[upi_tag.len()..trimmed.len() - 1];
                let png_b64 = render_qrcode(uri, QrErrorCorrection::M, 4).ok();
                job.elements.push(PrintElement::QrCode {
                    data: uri.to_string(),
                    error_correction: QrErrorCorrection::M,
                    module_size: 4,
                    png_b64,
                });
                let mut caption_style = self.printer.current_style();
                caption_style.alignment = Alignment::Center;
                caption_style.bold = true;
                job.elements.push(PrintElement::Text {
                    content: "SCAN TO PAY VIA UPI".to_string(),
                    style: caption_style,
                });
            } else if trimmed.starts_with(qr_tag) && trimmed.ends_with(']') {
                let uri = &trimmed[qr_tag.len()..trimmed.len() - 1];
                let png_b64 = render_qrcode(uri, QrErrorCorrection::M, 4).ok();
                job.elements.push(PrintElement::QrCode {
                    data: uri.to_string(),
                    error_correction: QrErrorCorrection::M,
                    module_size: 4,
                    png_b64,
                });
            } else if trimmed.starts_with(qrcode_tag) && trimmed.ends_with(']') {
                let uri = &trimmed[qrcode_tag.len()..trimmed.len() - 1];
                let png_b64 = render_qrcode(uri, QrErrorCorrection::M, 4).ok();
                job.elements.push(PrintElement::QrCode {
                    data: uri.to_string(),
                    error_correction: QrErrorCorrection::M,
                    module_size: 4,
                    png_b64,
                });
            } else if !line.is_empty() {
                let style = self.printer.current_style();
                job.elements.push(PrintElement::Text {
                    content: line.to_string(),
                    style,
                });
            }
        }
    }

    fn emit_barcode(&mut self, btype: BarcodeType, raw: Vec<u8>, job: &mut PrintJob) {
        let data = String::from_utf8_lossy(&raw).to_string();
        if btype == BarcodeType::QrCode {
            self.flush_text(job);
            let png_b64 = render_qrcode(&data, self.qr_ec, self.qr_size)
                .map_err(|e| self.warn(job, &format!("QR render failed: {}", e)))
                .ok();
            self.log_cmd(job, &[], &format!("GS k QR code data={:?}", data));
            job.elements.push(PrintElement::QrCode {
                data,
                error_correction: self.qr_ec,
                module_size: self.qr_size,
                png_b64,
            });
            return;
        }

        let png_b64 = render_barcode(&btype, &data)
            .map_err(|e| {
                self.warn(job, &format!("Barcode render failed ({:?} {:?}): {}", btype, data, e));
            })
            .ok();
        self.flush_text(job);
        self.log_cmd(job, &[], &format!("Barcode {:?} data={:?}", btype, data));
        job.elements.push(PrintElement::Barcode {
            barcode_type: btype,
            data,
            hri: self.bc_hri,
            height_dots: self.bc_height,
            png_b64,
        });
    }

    fn emit_qr(&mut self, job: &mut PrintJob) {
        if self.qr_data.is_empty() {
            self.warn(job, "QR print: no data stored");
            return;
        }
        let data = String::from_utf8_lossy(&self.qr_data).to_string();
        let ec   = self.qr_ec;
        let size = self.qr_size;
        let png_b64 = render_qrcode(&data, ec, size)
            .map_err(|e| self.warn(job, &format!("QR render failed: {}", e)))
            .ok();
        self.log_cmd(job, &[], &format!("QR code data={:?}", data));
        job.elements.push(PrintElement::QrCode {
            data,
            error_correction: ec,
            module_size: size,
            png_b64,
        });
        self.qr_data.clear();
    }

    fn emit_raster(&mut self, width_bytes: u32, height: u32, raw: Vec<u8>, job: &mut PrintJob) {
        use base64::Engine;
        let width_dots = width_bytes * 8;
        let mut px: Vec<u8> = Vec::with_capacity((width_dots * height) as usize);
        for row in 0..height {
            for col_byte in 0..width_bytes {
                let byte = raw.get((row * width_bytes + col_byte) as usize).copied().unwrap_or(0);
                for bit in (0..8).rev() {
                    px.push(if byte & (1 << bit) != 0 { 0 } else { 255 });
                }
            }
        }
        let png_b64 = image::GrayImage::from_raw(width_dots, height, px)
            .map(|img| {
                let mut buf = Vec::new();
                let _ = image::DynamicImage::ImageLuma8(img)
                    .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png);
                base64::engine::general_purpose::STANDARD.encode(&buf)
            })
            .unwrap_or_default();

        self.flush_text(job);
        self.log_cmd(job, &[], &format!("Raster image {}x{}", width_dots, height));
        if !png_b64.is_empty() {
            job.elements.push(PrintElement::Image(RasterImage {
                width_dots,
                height_dots: height,
                png_b64,
            }));
        }
    }

    // ── Utilities ─────────────────────────────────────────────────────────────

    fn decode_high(&self, b: u8) -> char {
        match self.printer.code_page {
            CodePage::Windows1252 | CodePage::Pc850 => char::from_u32(b as u32).unwrap_or('?'),
            _ => '?',
        }
    }

    fn log_cmd(&self, job: &mut PrintJob, bytes: &[u8], desc: &str) {
        debug!("[+{:04X}] {}", self.offset, desc);
        job.parsed_commands.push(ParsedCommand {
            byte_offset: self.offset,
            raw_bytes: bytes.to_vec(),
            description: desc.to_string(),
        });
    }

    fn warn(&self, job: &mut PrintJob, msg: &str) {
        warn!("[+{:04X}] {}", self.offset, msg);
        job.warnings.push(format!("[+{:04X}] {}", self.offset, msg));
    }
}

/// Convenience: parse raw bytes into a new PrintJob (used by commands.rs and tests)
pub fn parse_escpos(raw: Vec<u8>, paper_width: PaperWidth, job_id: String) -> PrintJob {
    use crate::print_job::JobSource;
    let mut job = PrintJob::new(job_id, JobSource::DirectApi, raw);
    let mut parser = EscPosParser::new(paper_width);
    parser.parse(&mut job);
    job
}
