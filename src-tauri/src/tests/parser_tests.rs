// tests/parser_tests.rs
// Automated Rust unit tests for the ESC/POS parser.
// Covers all 12 required test cases.

#[cfg(test)]
mod parser_tests {
    use crate::escpos_parser::parse_escpos;
    use crate::print_job::{BarcodeType, PrintElement, QrErrorCorrection};
    use crate::printer_state::{Alignment, PaperWidth};

    const ESC: u8 = 0x1B;
    const GS: u8 = 0x1D;
    const LF: u8 = 0x0A;

    fn job_id() -> String { "test-job".to_string() }

    fn text_elements(job: &crate::print_job::PrintJob) -> Vec<String> {
        job.elements.iter().filter_map(|e| match e {
            PrintElement::Text { content, .. } => Some(content.clone()),
            _ => None,
        }).collect()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 1: Basic text
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_basic_text() {
        let raw = b"Hello World".to_vec();
        let raw_with_lf = [raw.as_slice(), &[LF]].concat();
        let job = parse_escpos(raw_with_lf, PaperWidth::Mm80, job_id());
        let texts = text_elements(&job);
        assert!(!texts.is_empty(), "Should have at least one text element");
        assert_eq!(texts[0], "Hello World");
        assert!(job.warnings.is_empty() || job.warnings.iter().all(|w| !w.contains("panic")));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 2: Bold on/off
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_bold() {
        let raw = [
            &[ESC, b'E', 1] as &[u8],
            b"BOLD TEXT",
            &[LF],
            &[ESC, b'E', 0],
            b"normal",
            &[LF],
        ].concat();
        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        let mut found_bold = false;
        let mut found_normal = false;
        for el in &job.elements {
            if let PrintElement::Text { content, style } = el {
                if content == "BOLD TEXT" && style.bold {
                    found_bold = true;
                }
                if content == "normal" && !style.bold {
                    found_normal = true;
                }
            }
        }
        assert!(found_bold, "Should find bold text element");
        assert!(found_normal, "Should find non-bold text element");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 3: Center alignment
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_center_alignment() {
        let raw = [
            &[ESC, b'a', 1] as &[u8],
            b"CENTERED",
            &[LF],
        ].concat();
        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        let found = job.elements.iter().any(|e| matches!(e,
            PrintElement::Text { content, style }
            if content == "CENTERED" && style.alignment == Alignment::Center
        ));
        assert!(found, "Should find center-aligned text");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 4: Right alignment
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_right_alignment() {
        let raw = [
            &[ESC, b'a', 2] as &[u8],
            b"RIGHT",
            &[LF],
        ].concat();
        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        let found = job.elements.iter().any(|e| matches!(e,
            PrintElement::Text { content, style }
            if content == "RIGHT" && style.alignment == Alignment::Right
        ));
        assert!(found, "Should find right-aligned text");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 5: Line feed
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_line_feed() {
        let raw = vec![b'A', LF, b'B', LF];
        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        let feeds: Vec<_> = job.elements.iter().filter(|e| matches!(e, PrintElement::LineFeed { .. })).collect();
        assert!(feeds.len() >= 2, "Should have at least 2 line feed elements");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 6: 80mm receipt formatting
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_80mm_receipt() {
        let raw = crate::test_receipt::build_test_receipt();
        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        assert!(!job.elements.is_empty(), "Should have elements");
        assert!(job.byte_count > 0, "Should have bytes");

        // Should have a cut at the end
        let has_cut = job.elements.iter().any(|e| matches!(e, PrintElement::Cut { .. }));
        assert!(has_cut, "80mm receipt should have a cut");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 7: QR command
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_qr_command() {
        let qr_data = b"https://aqnoor.pharmacy/test";
        let data_len = (qr_data.len() as u16 + 3) as u16;
        let pl = (data_len & 0xFF) as u8;
        let ph = (data_len >> 8) as u8;

        let raw: Vec<u8> = [
            // Store
            &[GS, b'(', b'k', pl, ph, 49, 80, 0] as &[u8],
            qr_data.as_ref(),
            // Print
            &[GS, b'(', b'k', 3, 0, 49, 81, 0],
            &[LF],
        ].concat();

        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        let found = job.elements.iter().any(|e| matches!(e, PrintElement::QrCode { data, .. } if data.contains("aqnoor")));
        assert!(found, "Should have QR code element with correct data");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 8: Barcode command
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_barcode_command() {
        // GS k 73 (CODE128 new format) len=10 data="TEST123456"
        let bc_data = b"TEST123456";
        let raw: Vec<u8> = [
            &[GS, b'k', 73, bc_data.len() as u8] as &[u8],
            bc_data.as_ref(),
            &[LF],
        ].concat();

        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        let found = job.elements.iter().any(|e| matches!(e, PrintElement::Barcode { data, .. } if data == "TEST123456"));
        assert!(found, "Should have barcode element with data");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 9: Cut command
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_cut_command() {
        // GS V 0 (full cut)
        let raw = vec![GS, b'V', 0];
        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        let found = job.elements.iter().any(|e| matches!(e, PrintElement::Cut { partial: false }));
        assert!(found, "Should have full cut element");

        // GS V 1 (partial cut)
        let raw2 = vec![GS, b'V', 1];
        let job2 = parse_escpos(raw2, PaperWidth::Mm80, job_id());
        let found2 = job2.elements.iter().any(|e| matches!(e, PrintElement::Cut { partial: true }));
        assert!(found2, "Should have partial cut element");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 10: Multiple print jobs (parser reuse)
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_multiple_print_jobs() {
        let r1 = [b"Job One".as_ref(), &[LF], &[GS, b'V', 0]].concat();
        let r2 = [b"Job Two".as_ref(), &[LF], &[GS, b'V', 0]].concat();

        let j1 = parse_escpos(r1, PaperWidth::Mm80, "job-1".to_string());
        let j2 = parse_escpos(r2, PaperWidth::Mm80, "job-2".to_string());

        assert_eq!(text_elements(&j1), vec!["Job One"]);
        assert_eq!(text_elements(&j2), vec!["Job Two"]);
        // Parser state does not leak between jobs
        assert_eq!(j1.id, "job-1");
        assert_eq!(j2.id, "job-2");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 11: Invalid / unsupported commands (must not crash)
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_invalid_commands_no_crash() {
        let raw = vec![
            ESC, 0x7F, // unknown ESC command
            GS, 0x7F,  // unknown GS command
            0x01,      // SOH — control char
            0x02,      // STX
            b'A',      // valid char should still be parsed
            LF,
        ];
        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        // Should not panic, should have warnings
        assert!(!job.warnings.is_empty(), "Should have warnings for unknown commands");

        // Should still emit the valid 'A' character
        let texts = text_elements(&job);
        assert!(texts.iter().any(|t| t.contains('A')), "Should still parse valid chars after bad commands");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test 12: Mixed text + formatting + QR + cut
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_mixed_receipt() {
        let qr_payload = b"QR_DATA_MIXED";
        let data_len = (qr_payload.len() as u16 + 3) as u16;
        let pl = (data_len & 0xFF) as u8;
        let ph = (data_len >> 8) as u8;

        let raw: Vec<u8> = [
            // Init
            &[ESC, b'@'] as &[u8],
            // Center bold heading
            &[ESC, b'a', 1],
            &[ESC, b'E', 1],
            b"PHARMACY",
            &[LF],
            &[ESC, b'E', 0],
            // Left-aligned items
            &[ESC, b'a', 0],
            b"Medicine    100.00",
            &[LF],
            // Right-aligned total
            &[ESC, b'a', 2],
            &[ESC, b'E', 1],
            b"TOTAL: 100.00",
            &[LF],
            &[ESC, b'E', 0],
            // QR code
            &[ESC, b'a', 1],
            &[GS, b'(', b'k', pl, ph, 49, 80, 0],
            qr_payload.as_ref(),
            &[GS, b'(', b'k', 3, 0, 49, 81, 0],
            &[LF],
            // Full cut
            &[GS, b'V', 0],
        ].concat();

        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        // Should have text elements
        assert!(text_elements(&job).iter().any(|t| t.contains("PHARMACY")));
        assert!(text_elements(&job).iter().any(|t| t.contains("Medicine")));
        assert!(text_elements(&job).iter().any(|t| t.contains("TOTAL")));

        // Should have QR code
        assert!(job.elements.iter().any(|e| matches!(e, PrintElement::QrCode { .. })));

        // Should have cut
        assert!(job.elements.iter().any(|e| matches!(e, PrintElement::Cut { .. })));

        // No panics, warnings only for unknown if any
        println!("Warnings: {:?}", job.warnings);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Bonus: ESC @ reset test
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_esc_at_reset() {
        let raw = [
            &[ESC, b'E', 1] as &[u8], // bold on
            b"BOLD",
            &[LF],
            &[ESC, b'@'],              // reset
            b"NORMAL",
            &[LF],
        ].concat();
        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        let mut found_normal = false;
        for el in &job.elements {
            if let PrintElement::Text { content, style } = el {
                if content == "NORMAL" {
                    assert!(!style.bold, "After ESC @ reset, text should not be bold");
                    found_normal = true;
                }
            }
        }
        assert!(found_normal, "Should find NORMAL text after reset");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Bonus: ESC d feed lines
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_esc_d_feed_lines() {
        let raw = vec![ESC, b'd', 5]; // feed 5 lines
        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        let found = job.elements.iter().any(|e| matches!(e, PrintElement::LineFeed { lines: 5 }));
        assert!(found, "Should have LineFeed {{ lines: 5 }} element");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Bonus: Underline
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_underline() {
        let raw = [
            &[ESC, b'-', 1] as &[u8],
            b"underlined",
            &[LF],
        ].concat();
        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        let found = job.elements.iter().any(|e| matches!(e,
            PrintElement::Text { content, style } if content == "underlined" && style.underline > 0
        ));
        assert!(found, "Should find underlined text");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Bonus: GS ! char size
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_gs_char_size() {
        let raw = [
            &[GS, b'!', 0x11] as &[u8], // width x2, height x2
            b"BIG",
            &[LF],
        ].concat();
        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());

        let found = job.elements.iter().any(|e| matches!(e,
            PrintElement::Text { content, style }
            if content == "BIG" && style.char_width_multiplier == 2 && style.char_height_multiplier == 2
        ));
        assert!(found, "Should find double-size text");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test: ESC Z QR Code
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_esc_z_qr_command() {
        let qr_data = b"https://example.com/qr-esc-z";
        let len = qr_data.len() as u16;
        let xl = (len & 0xFF) as u8;
        let xh = (len >> 8) as u8;

        let raw: Vec<u8> = [
            &[ESC, b'Z', 0, b'M', 4, xl, xh] as &[u8],
            qr_data.as_ref(),
            &[LF],
        ].concat();

        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());
        let found = job.elements.iter().any(|e| matches!(e, PrintElement::QrCode { data, .. } if data.contains("example.com")));
        assert!(found, "Should parse ESC Z QR code");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test: GS k QR Code
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_gs_k_qr_command() {
        let qr_data = b"https://example.com/gs-k-qr";
        let raw: Vec<u8> = [
            &[GS, b'k', 104, qr_data.len() as u8] as &[u8],
            qr_data.as_ref(),
            &[LF],
        ].concat();

        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());
        let found = job.elements.iter().any(|e| matches!(e, PrintElement::QrCode { data, .. } if data.contains("gs-k-qr")));
        assert!(found, "Should parse GS k 104 QR code");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test: Embedded [UPI_QR:...] tag in text
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_embedded_upi_qr_tag() {
        let text = b"Total: 150.00\n[UPI_QR:upi://pay?pa=pharmapos@upi&pn=PHARMA%20POS&am=150.00]\nThank you!\n";
        let raw = [
            &[ESC, b'@'] as &[u8],
            text.as_ref(),
            &[GS, b'V', 0],
        ].concat();

        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());
        let qr_found = job.elements.iter().any(|e| matches!(e, PrintElement::QrCode { data, .. } if data.contains("pharmapos@upi")));
        assert!(qr_found, "Should parse embedded UPI_QR tag into visual QR code element");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Test: GS v 0 Raster Image (as generated by PharmaPOS for QR codes)
    // ─────────────────────────────────────────────────────────────────────────
    #[test]
    fn test_gs_v_0_raster_qr() {
        let width_bytes: u16 = 4; // 32 dots
        let height_dots: u16 = 32;
        let xl = (width_bytes & 0xFF) as u8;
        let xh = (width_bytes >> 8) as u8;
        let yl = (height_dots & 0xFF) as u8;
        let yh = (height_dots >> 8) as u8;

        let raster_len = (width_bytes as usize) * (height_dots as usize);
        let raster_data = vec![0xAAu8; raster_len];

        let raw: Vec<u8> = [
            &[ESC, b'@'] as &[u8],
            &[GS, b'v', b'0', 0, xl, xh, yl, yh],
            &raster_data,
            b"\nSCAN TO PAY VIA UPI\n",
            &[GS, b'V', 0],
        ].concat();

        let job = parse_escpos(raw, PaperWidth::Mm80, job_id());
        let image_found = job.elements.iter().any(|e| matches!(e, PrintElement::Image(..)));
        assert!(image_found, "Should parse GS v 0 raster image into an Image element");
    }
}
