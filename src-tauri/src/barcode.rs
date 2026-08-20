// barcode.rs
// Renders barcode data to PNG bytes (base64-encoded) using the rxing crate.
// Supports CODE128, CODE39, EAN13, EAN8, UPC-A, ITF, Codabar, Code93.

use crate::print_job::BarcodeType;
use base64::Engine;
use rxing::{
    BarcodeFormat, EncodingHintDictionary, MultiFormatWriter, Writer,
};
use std::collections::HashMap;

/// Render a barcode to a base64-encoded PNG string.
/// Returns Err if the barcode data is invalid for the requested type.
pub fn render_barcode(btype: &BarcodeType, data: &str) -> Result<String, String> {
    let format = btype_to_format(btype)?;

    let width: u32 = 400;
    let height: u32 = 120;

    let hints: EncodingHintDictionary = HashMap::new();
    let writer = MultiFormatWriter;

    let bit_matrix = writer
        .encode_with_hints(data, &format, width as i32, height as i32, &hints)
        .map_err(|e| format!("Barcode encode error: {:?}", e))?;

    // Convert BitMatrix → grayscale image
    let bm_width  = bit_matrix.width()  as u32;
    let bm_height = bit_matrix.height() as u32;
    let mut img = image::GrayImage::new(bm_width, bm_height);

    for y in 0..bm_height {
        for x in 0..bm_width {
            let px = if bit_matrix.get(x, y) { 0u8 } else { 255u8 };
            img.put_pixel(x, y, image::Luma([px]));
        }
    }

    let mut png_buf = Vec::new();
    image::DynamicImage::ImageLuma8(img)
        .write_to(&mut std::io::Cursor::new(&mut png_buf), image::ImageFormat::Png)
        .map_err(|e| format!("PNG encode error: {}", e))?;

    Ok(base64::engine::general_purpose::STANDARD.encode(&png_buf))
}

fn btype_to_format(btype: &BarcodeType) -> Result<BarcodeFormat, String> {
    match btype {
        BarcodeType::UpcA     => Ok(BarcodeFormat::UPC_A),
        BarcodeType::UpcE     => Ok(BarcodeFormat::UPC_E),
        BarcodeType::Ean13    => Ok(BarcodeFormat::EAN_13),
        BarcodeType::Ean8     => Ok(BarcodeFormat::EAN_8),
        BarcodeType::Code39   => Ok(BarcodeFormat::CODE_39),
        BarcodeType::Itf      => Ok(BarcodeFormat::ITF),
        BarcodeType::Codabar  => Ok(BarcodeFormat::CODABAR),
        BarcodeType::Code93   => Ok(BarcodeFormat::CODE_93),
        BarcodeType::Code128  => Ok(BarcodeFormat::CODE_128),
        BarcodeType::Pdf417   => Ok(BarcodeFormat::PDF_417),
        BarcodeType::QrCode   => Ok(BarcodeFormat::QR_CODE),
        BarcodeType::Unknown(n) => Err(format!("Unknown barcode type 0x{:02X}", n)),
    }
}
