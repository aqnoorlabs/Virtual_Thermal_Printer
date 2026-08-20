// renderer.rs
// Converts a PrintJob into an HTML string suitable for display in the Tauri webview.
// This is a pure transformation — no I/O, no UI coupling.
//
// The output is an HTML fragment representing an 80mm (or 58mm) thermal receipt.
// Embedded images (barcodes, QR, rasters) are base64 data URIs.

use crate::print_job::{HriPosition, PrintElement, PrintJob};
use crate::printer_state::{Alignment, Font, PaperWidth};

/// Width constants (in CSS pixels at 96dpi — we treat 1 dot ≈ 0.5px for readability)
const PX_PER_DOT: f32 = 0.5;

pub struct ReceiptRenderer;

impl ReceiptRenderer {
    /// Render a PrintJob into a self-contained HTML string.
    pub fn render_html(job: &PrintJob, paper_width: PaperWidth) -> String {
        let printable_dots = paper_width.printable_px();
        let receipt_px = (printable_dots as f32 * PX_PER_DOT) as u32;

        let mut body = String::new();
        let mut _in_cut = false;

        for element in &job.elements {
            match element {
                PrintElement::Text { content, style } => {
                    let align_css = match style.alignment {
                        Alignment::Left   => "left",
                        Alignment::Center => "center",
                        Alignment::Right  => "right",
                    };
                    let font_family = match style.font {
                        Font::A => "'Courier New', Courier, monospace",
                        Font::B => "'Courier New', Courier, monospace",
                    };
                    let font_size_base: f32 = match style.font {
                        Font::A => 12.0,
                        Font::B => 10.0,
                    };
                    let font_size = font_size_base * style.char_height_multiplier as f32;
                    let letter_spacing = if style.char_width_multiplier > 1 {
                        format!("letter-spacing: {}px;", (style.char_width_multiplier - 1) * 4)
                    } else {
                        String::new()
                    };

                    let mut span_style = format!(
                        "font-family:{font_family}; font-size:{font_size}px; text-align:{align_css}; display:block; white-space:pre-wrap; word-break:break-all; {letter_spacing}"
                    );
                    if style.bold         { span_style.push_str(" font-weight:bold;"); }
                    if style.underline > 0 { span_style.push_str(" text-decoration:underline;"); }
                    if style.inverse {
                        span_style.push_str(" background:#000; color:#fff;");
                    }

                    let escaped = html_escape(content);
                    body.push_str(&format!(
                        r#"<div class="receipt-line" style="{span_style}">{escaped}</div>"#
                    ));
                }

                PrintElement::LineFeed { lines } => {
                    for _ in 0..*lines {
                        body.push_str(r#"<div class="receipt-feed"></div>"#);
                    }
                }

                PrintElement::FeedDots { dots } => {
                    let px = (*dots as f32 * PX_PER_DOT) as u32;
                    body.push_str(&format!(
                        r#"<div class="receipt-feed" style="height:{}px;"></div>"#,
                        px
                    ));
                }

                PrintElement::Barcode { data, png_b64, hri, height_dots, .. } => {
                    let h_px = (*height_dots as f32 * PX_PER_DOT) as u32;
                    body.push_str(r#"<div class="receipt-barcode">"#);
                    if let Some(b64) = png_b64 {
                        body.push_str(&format!(
                            r#"<img src="data:image/png;base64,{b64}" style="max-width:100%; height:{h_px}px; display:block; margin:4px auto;" alt="barcode"/>"#
                        ));
                    } else {
                        body.push_str(&format!(
                            r#"<div class="barcode-fallback">[Barcode: {}]</div>"#,
                            html_escape(data)
                        ));
                    }
                    match hri {
                        HriPosition::Below | HriPosition::Both => {
                            body.push_str(&format!(
                                r#"<div class="barcode-hri">{}</div>"#,
                                html_escape(data)
                            ));
                        }
                        _ => {}
                    }
                    body.push_str("</div>");
                }

                PrintElement::QrCode { data, png_b64, .. } => {
                    body.push_str(r#"<div class="receipt-qr">"#);
                    if let Some(b64) = png_b64 {
                        body.push_str(&format!(
                            r#"<img src="data:image/png;base64,{b64}" style="max-width:180px; display:block; margin:4px auto;" alt="qr code"/>"#
                        ));
                    } else {
                        body.push_str(&format!(
                            r#"<div class="qr-fallback">[QR: {}]</div>"#,
                            html_escape(data)
                        ));
                    }
                    body.push_str("</div>");
                }

                PrintElement::Image(raster) => {
                    let w_px = (raster.width_dots as f32 * PX_PER_DOT) as u32;
                    body.push_str(&format!(
                        r#"<div class="receipt-image"><img src="data:image/png;base64,{}" style="max-width:{}px; display:block; margin:4px auto;" alt="print image"/></div>"#,
                        raster.png_b64, w_px
                    ));
                }

                PrintElement::Cut { partial } => {
                    let label = if *partial { "— — — — — CUT — — — — —" } else { "━━━━━━━━━━ CUT ━━━━━━━━━━" };
                    body.push_str(&format!(
                        r#"<div class="receipt-cut {}"><span>{}</span></div>"#,
                        if *partial { "partial" } else { "full" },
                        label
                    ));
                    _in_cut = true;
                }

                PrintElement::Rule => {
                    body.push_str(r#"<div class="receipt-rule"><hr/></div>"#);
                }
            }
        }

        // Wrap in receipt container
        let meta = format!(
            "Job ID: {} | {} bytes | {} | {}",
            &job.id[..8.min(job.id.len())],
            job.byte_count,
            match paper_width { PaperWidth::Mm80 => "80mm", PaperWidth::Mm58 => "58mm" },
            job.received_at.format("%H:%M:%S")
        );

        format!(r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8"/>
<style>
* {{ box-sizing: border-box; margin: 0; padding: 0; }}
body {{ background: #1a1a1a; display: flex; flex-direction: column; align-items: center; padding: 16px; font-family: 'Courier New', monospace; }}
.receipt-wrapper {{ background: #fff; width: {receipt_px}px; padding: 8px 12px; border-radius: 4px; box-shadow: 0 4px 24px rgba(0,0,0,0.6); }}
.receipt-meta {{ font-size: 9px; color: #999; text-align: center; padding: 2px 0 6px 0; border-bottom: 1px dashed #ddd; margin-bottom: 4px; }}
.receipt-line {{ min-height: 1em; line-height: 1.3; padding: 0; }}
.receipt-feed {{ height: 6px; }}
.receipt-cut {{ text-align: center; font-size: 10px; color: #888; margin: 8px 0; letter-spacing: 2px; }}
.receipt-cut.full span {{ color: #555; }}
.receipt-cut.partial span {{ color: #888; font-style: italic; }}
.receipt-barcode {{ text-align: center; margin: 6px 0; }}
.barcode-hri {{ font-size: 10px; text-align: center; letter-spacing: 2px; margin-top: 2px; }}
.barcode-fallback {{ font-size: 10px; color: #888; border: 1px dashed #ccc; padding: 4px; text-align: center; }}
.receipt-qr {{ text-align: center; margin: 6px 0; }}
.qr-fallback {{ font-size: 10px; color: #888; border: 1px dashed #ccc; padding: 4px; text-align: center; }}
.receipt-image {{ text-align: center; margin: 6px 0; }}
.receipt-rule hr {{ border: none; border-top: 1px solid #333; }}
</style>
</head>
<body>
<div class="receipt-wrapper">
<div class="receipt-meta">{meta}</div>
{body}
</div>
</body>
</html>"#)
    }

    /// Render PrintJob as PNG bytes (using the HTML approach with embedded images).
    /// For a real raster render we'd need a headless browser or custom rasterizer;
    /// here we produce a simple text-only PNG for the "save" feature.
    pub fn render_png_bytes(_job: &PrintJob, _paper_width: PaperWidth) -> Vec<u8> {
        // Stub: actual PNG rasterization would require a headless WebKit/WebView
        // which is complex on Windows without additional dependencies.
        // The save-as-PNG in the frontend can use html2canvas or similar.
        Vec::new()
    }
}

/// Escape HTML special characters
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&#39;")
}
