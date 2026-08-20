// test_receipt.rs
// Builds a sample AqNoor Pharmacy ESC/POS receipt as raw bytes.
// Use this to test the parser and preview without a real POS application.

use crate::printer_state::Alignment;

const ESC: u8 = 0x1B;
const GS:  u8 = 0x1D;
const LF:  u8 = 0x0A;

/// Build the full test receipt as a Vec<u8> of raw ESC/POS bytes.
pub fn build_test_receipt() -> Vec<u8> {
    let mut data: Vec<u8> = Vec::new();

    // ── Initialize ────────────────────────────────────────────────────────────
    data.extend_from_slice(&[ESC, b'@']);

    // ── Header: centered pharmacy name (bold + double size) ───────────────────
    data.extend_from_slice(&[ESC, b'a', 1]); // center
    data.extend_from_slice(&[GS, b'!', 0x11]); // double width + double height
    data.extend_from_slice(&[ESC, b'E', 1]); // bold on
    data.extend_from_slice(b"AQNOOR PHARMACY");
    data.push(LF);
    data.extend_from_slice(&[GS, b'!', 0x00]); // reset size
    data.extend_from_slice(&[ESC, b'E', 0]); // bold off

    // Sub-header
    data.extend_from_slice(b"123 Main Street, Pharmacy Lane");
    data.push(LF);
    data.extend_from_slice(b"Tel: +92-300-0000000");
    data.push(LF);

    // Separator
    data.extend_from_slice(&[ESC, b'a', 0]); // left
    data.extend_from_slice(b"--------------------------------");
    data.push(LF);

    // Date / receipt number
    data.extend_from_slice(&[ESC, b'a', 0]); // left
    data.extend_from_slice(b"Date: 2026-08-20  Time: 10:15");
    data.push(LF);
    data.extend_from_slice(b"Receipt#: 00042");
    data.push(LF);
    data.extend_from_slice(b"--------------------------------");
    data.push(LF);

    // ── Items ─────────────────────────────────────────────────────────────────
    data.extend_from_slice(b"ITEM                       QTY   PRICE");
    data.push(LF);
    data.extend_from_slice(b"Paracetamol 500mg            2   50.00");
    data.push(LF);
    data.extend_from_slice(b"Amoxicillin 500mg            1   80.00");
    data.push(LF);
    data.extend_from_slice(b"ORS Sachet                   3   25.00");
    data.push(LF);
    data.extend_from_slice(b"--------------------------------");
    data.push(LF);

    // ── Totals ────────────────────────────────────────────────────────────────
    data.extend_from_slice(&[ESC, b'a', 0]); // left
    data.extend_from_slice(b"Subtotal               155.00");
    data.push(LF);
    data.extend_from_slice(b"GST (6.45%)             10.00");
    data.push(LF);
    data.extend_from_slice(b"--------------------------------");
    data.push(LF);

    // Bold TOTAL (double height)
    data.extend_from_slice(&[ESC, b'E', 1]);
    data.extend_from_slice(&[GS, b'!', 0x01]); // double height only
    data.extend_from_slice(b"TOTAL                  165.00");
    data.push(LF);
    data.extend_from_slice(&[GS, b'!', 0x00]);
    data.extend_from_slice(&[ESC, b'E', 0]);

    // Payment method
    data.extend_from_slice(b"Payment: Cash");
    data.push(LF);
    data.extend_from_slice(b"Tendered:              200.00");
    data.push(LF);
    data.extend_from_slice(b"Change:                 35.00");
    data.push(LF);
    data.extend_from_slice(b"--------------------------------");
    data.push(LF);

    // ── QR Code ───────────────────────────────────────────────────────────────
    data.extend_from_slice(&[ESC, b'a', 1]); // center

    // GS ( k — QR: set model 2
    // pL=4, pH=0, cn=49, fn=65, n1=50 (model 2), n2=0
    data.extend_from_slice(&[GS, b'(', b'k', 4, 0, 49, 65, 50, 0]);

    // GS ( k — QR: set size (module size = 4)
    // pL=3, pH=0, cn=49, fn=67, n=4
    data.extend_from_slice(&[GS, b'(', b'k', 3, 0, 49, 67, 4]);

    // GS ( k — QR: set error correction (M = 49)
    // pL=3, pH=0, cn=49, fn=69, n=49
    data.extend_from_slice(&[GS, b'(', b'k', 3, 0, 49, 69, 49]);

    // GS ( k — QR: store data
    // data = "https://aqnoor.pharmacy/rx/00042"
    let qr_data = b"https://aqnoor.pharmacy/rx/00042";
    let qr_data_len = qr_data.len() as u16 + 3; // +3 for cn, fn, m
    let qr_pl = (qr_data_len & 0xFF) as u8;
    let qr_ph = (qr_data_len >> 8) as u8;
    // pL, pH, cn=49, fn=80, m=0, data...
    data.extend_from_slice(&[GS, b'(', b'k', qr_pl, qr_ph, 49, 80, 0]);
    data.extend_from_slice(qr_data);

    // GS ( k — QR: print
    // pL=3, pH=0, cn=49, fn=81, m=0
    data.extend_from_slice(&[GS, b'(', b'k', 3, 0, 49, 81, 0]);

    data.push(LF);

    // ── Barcode (CODE128) ─────────────────────────────────────────────────────
    // GS h — barcode height = 80 dots
    data.extend_from_slice(&[GS, b'h', 80]);
    // GS w — barcode width = 2
    data.extend_from_slice(&[GS, b'w', 2]);
    // GS H — HRI below
    data.extend_from_slice(&[GS, b'H', 2]);
    // GS k 73 (CODE128, new format) n=10 data="00042RX001"
    let bc_data = b"00042RX001";
    data.extend_from_slice(&[GS, b'k', 73, bc_data.len() as u8]);
    data.extend_from_slice(bc_data);

    data.push(LF);

    // ── Footer ────────────────────────────────────────────────────────────────
    data.extend_from_slice(&[ESC, b'a', 1]); // center
    data.extend_from_slice(b"Thank you for choosing AqNoor!");
    data.push(LF);
    data.extend_from_slice(b"Get well soon.");
    data.push(LF);
    data.extend_from_slice(b"www.aqnoor.pharmacy");
    data.push(LF);

    // Feed and cut
    data.extend_from_slice(&[ESC, b'd', 3]); // feed 3 lines
    data.extend_from_slice(&[GS, b'V', 0]);  // full cut

    data
}

/// Build a minimal test receipt for unit tests
pub fn build_minimal_receipt() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&[ESC, b'@']);
    data.extend_from_slice(&[ESC, b'a', 1]); // center
    data.extend_from_slice(&[ESC, b'E', 1]); // bold
    data.extend_from_slice(b"TEST");
    data.push(LF);
    data.extend_from_slice(&[ESC, b'E', 0]);
    data.extend_from_slice(&[GS, b'V', 0]); // cut
    data
}

/// Build raw bytes for a text-only receipt with right-aligned total
pub fn build_text_receipt(lines: &[(&str, Alignment)]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&[ESC, b'@']);
    for (text, align) in lines {
        let a = match align {
            Alignment::Left   => 0,
            Alignment::Center => 1,
            Alignment::Right  => 2,
        };
        data.extend_from_slice(&[ESC, b'a', a]);
        data.extend_from_slice(text.as_bytes());
        data.push(LF);
    }
    data.extend_from_slice(&[GS, b'V', 0]);
    data
}

/// Build a sample receipt featuring UPI QR code payment
pub fn build_upi_test_receipt() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&[ESC, b'@']);
    data.extend_from_slice(&[ESC, b'a', 1]);
    data.extend_from_slice(&[GS, b'!', 0x11]);
    data.extend_from_slice(&[ESC, b'E', 1]);
    data.extend_from_slice(b"AQNOOR PHARMACY");
    data.push(LF);
    data.extend_from_slice(&[GS, b'!', 0x00]);
    data.extend_from_slice(&[ESC, b'E', 0]);
    data.extend_from_slice(b"123 Main Street, Pharmacy Lane\nTel: +92-300-0000000\n--------------------------------\n");
    data.extend_from_slice(b"Date: 2026-08-20  Time: 10:15\nReceipt#: 00042\n--------------------------------\n");
    data.extend_from_slice(b"ITEM                       QTY   PRICE\nParacetamol 500mg            2   50.00\nAmoxicillin 500mg            1   80.00\nORS Sachet                   3   25.00\n--------------------------------\n");
    data.extend_from_slice(b"Subtotal               155.00\nGST (6.45%)             10.00\n--------------------------------\nTOTAL                  165.00\n--------------------------------\n");

    // Add UPI QR Code tag
    let upi_uri = b"upi://pay?pa=aqnoor@upi&pn=AQNOOR%20PHARMACY&am=165.00&cu=INR&tn=RX-00042";
    data.extend_from_slice(b"[UPI_QR:");
    data.extend_from_slice(upi_uri);
    data.extend_from_slice(b"]\n");

    data.extend_from_slice(&[ESC, b'a', 1]);
    data.extend_from_slice(b"Thank you for choosing AqNoor!\nGet well soon.\n");
    data.extend_from_slice(&[ESC, b'd', 3]);
    data.extend_from_slice(&[GS, b'V', 0]);
    data
}

