// printer_state.rs
// Maintains the full mutable state of the virtual thermal printer.
// This is purely a data structure — no I/O or rendering.

use serde::{Deserialize, Serialize};

/// Paper width preset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PaperWidth {
    /// 80mm paper — 72mm printable — ~576 dots at 203 DPI
    #[default]
    Mm80,
    /// 58mm paper — 48mm printable — ~384 dots at 203 DPI
    Mm58,
}

impl PaperWidth {
    /// Printable width in pixels at 203 DPI
    pub fn printable_px(&self) -> u32 {
        match self {
            PaperWidth::Mm80 => 576,
            PaperWidth::Mm58 => 384,
        }
    }

    /// Character columns for font A (12px wide)
    pub fn char_cols_font_a(&self) -> u32 {
        self.printable_px() / 12
    }

    /// Character columns for font B (9px wide)
    pub fn char_cols_font_b(&self) -> u32 {
        self.printable_px() / 9
    }
}

/// Text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Font selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Font {
    /// Font A — standard 12×24 dot matrix
    #[default]
    A,
    /// Font B — smaller 9×17 dot matrix
    B,
}

/// Code page / character encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CodePage {
    #[default]
    Pc437,       // IBM Code Page 437 (default)
    Pc850,       // IBM Code Page 850
    Windows1252, // Windows-1252
    Utf8,        // UTF-8 passthrough
    Unknown(u8), // Unrecognised code page
}

/// Complete printer state — matches a real ESC/POS thermal printer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterState {
    // Paper
    pub paper_width: PaperWidth,

    // Text formatting
    pub alignment: Alignment,
    pub bold: bool,
    pub underline: u8,   // 0=off 1=single 2=double
    pub inverse: bool,   // GS B — white-on-black
    pub italic: bool,    // not all printers; track anyway
    pub double_strike: bool,

    // Font
    pub font: Font,
    pub char_width_multiplier: u8,  // 1–8 (GS ! upper nibble)
    pub char_height_multiplier: u8, // 1–8 (GS ! lower nibble)

    // Spacing
    pub line_spacing: u8, // dots between lines (default ~30)

    // Encoding
    pub code_page: CodePage,

    // Motion tracking (in dots)
    pub cursor_x: u32,
    pub cursor_y: u32,

    // Current text line buffer (collected until LF)
    pub line_buffer: String,

    // Rotation / upside-down (ESC {, ESC V)
    pub upside_down: bool,
    pub rotate_90: bool,
}

impl Default for PrinterState {
    fn default() -> Self {
        Self {
            paper_width: PaperWidth::default(),
            alignment: Alignment::default(),
            bold: false,
            underline: 0,
            inverse: false,
            italic: false,
            double_strike: false,
            font: Font::default(),
            char_width_multiplier: 1,
            char_height_multiplier: 1,
            line_spacing: 30,
            code_page: CodePage::default(),
            cursor_x: 0,
            cursor_y: 0,
            line_buffer: String::new(),
            upside_down: false,
            rotate_90: false,
        }
    }
}

impl PrinterState {
    /// Reset to factory defaults (ESC @)
    pub fn reset(&mut self) {
        *self = Self {
            paper_width: self.paper_width, // keep paper width setting
            ..Default::default()
        };
    }

    /// Snapshot of the current text style (used to stamp a PrintElement)
    pub fn current_style(&self) -> TextStyle {
        TextStyle {
            alignment: self.alignment,
            bold: self.bold,
            underline: self.underline,
            inverse: self.inverse,
            italic: self.italic,
            font: self.font,
            char_width_multiplier: self.char_width_multiplier,
            char_height_multiplier: self.char_height_multiplier,
            double_strike: self.double_strike,
        }
    }

    /// Characters per line for current font + width multiplier
    pub fn chars_per_line(&self) -> u32 {
        let base = match self.font {
            Font::A => self.paper_width.char_cols_font_a(),
            Font::B => self.paper_width.char_cols_font_b(),
        };
        // Width multiplier makes each char wider, so fewer fit
        (base / self.char_width_multiplier as u32).max(1)
    }
}

/// Snapshot of text formatting at the moment text was emitted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextStyle {
    pub alignment: Alignment,
    pub bold: bool,
    pub underline: u8,
    pub inverse: bool,
    pub italic: bool,
    pub font: Font,
    pub char_width_multiplier: u8,
    pub char_height_multiplier: u8,
    pub double_strike: bool,
}
