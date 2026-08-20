// qrcode.rs
// Renders QR code data to PNG bytes (base64-encoded) using the qrcode crate.

use crate::print_job::QrErrorCorrection;
use base64::Engine;
use qrcode::{EcLevel, QrCode};

/// Render a QR code to a base64-encoded PNG string.
/// module_size: pixels per QR module (1–16), controls the visual size.
pub fn render_qrcode(
    data: &str,
    ec: QrErrorCorrection,
    module_size: u8,
) -> Result<String, String> {
    let ec_level = match ec {
        QrErrorCorrection::L => EcLevel::L,
        QrErrorCorrection::M => EcLevel::M,
        QrErrorCorrection::Q => EcLevel::Q,
        QrErrorCorrection::H => EcLevel::H,
    };

    let code = QrCode::with_error_correction_level(data.as_bytes(), ec_level)
        .map_err(|e| format!("QR encode error: {}", e))?;

    let module_px = module_size.max(1).min(16) as u32;
    let quiet = module_px * 4; // 4-module quiet zone

    let modules = code.width() as u32;
    let img_size = modules * module_px + quiet * 2;

    let mut img = image::GrayImage::new(img_size, img_size);

    // Fill background white
    for px in img.pixels_mut() {
        *px = image::Luma([255u8]);
    }

    // Draw QR modules
    for row in 0..modules {
        for col in 0..modules {
            if code[(col as usize, row as usize)] == qrcode::Color::Dark {
                let x0 = quiet + col * module_px;
                let y0 = quiet + row * module_px;
                for dy in 0..module_px {
                    for dx in 0..module_px {
                        img.put_pixel(x0 + dx, y0 + dy, image::Luma([0u8]));
                    }
                }
            }
        }
    }

    let mut png_buf = Vec::new();
    image::DynamicImage::ImageLuma8(img)
        .write_to(&mut std::io::Cursor::new(&mut png_buf), image::ImageFormat::Png)
        .map_err(|e| format!("PNG encode error: {}", e))?;

    Ok(base64::engine::general_purpose::STANDARD.encode(&png_buf))
}
