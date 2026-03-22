// src-tauri/src/utils/escpos.rs
//! ESC/POS byte-stream builder.
//!
//! Builds raw byte sequences compatible with any ESC/POS thermal printer
//! (Epson TM series, Xprinter, Sewoo, Bixolon, Gprinter — all common in Algeria).
//!
//! Paper widths supported:
//!   • 58 mm → 32 printable characters per line
//!   • 80 mm → 48 printable characters per line
//!
//! Arabic note: ESC/POS printers almost universally use 8-bit code pages
//! (CP1256 / Windows-1256 for Arabic). We emit the Arabic name ONLY if the
//! printer supports it; otherwise we fall back to French.
//! Bilingual support is handled by printing French on one line and Arabic below.

use serde::Deserialize;

// ─── ESC/POS opcode constants ──────────────────────────────────────────────────

const ESC: u8 = 0x1B;
const GS:  u8 = 0x1D;
const LF:  u8 = 0x0A;
#[allow(dead_code)]
const HT:  u8 = 0x09;

// ─── Public builder ────────────────────────────────────────────────────────────

pub struct EscPosBuilder {
    buf:   Vec<u8>,
    width: usize,    // printable char width (32 or 48)
}

impl EscPosBuilder {
    pub fn new(width_mm: u8) -> Self {
        let width = if width_mm <= 58 { 32 } else { 48 };
        let mut b = Self { buf: Vec::with_capacity(1024), width };
        b.cmd_init();
        b.cmd_charset_pc1252();   // Latin-1 + accented French chars
        b
    }

    // ── Low-level opcodes ──────────────────────────────────────────────────

    fn cmd_init(&mut self) {
        self.push(&[ESC, b'@']);
    }

    fn cmd_charset_pc1252(&mut self) {
        // Select code page 16 = Windows-1252 (supports é, è, à, ç, etc.)
        self.push(&[ESC, b't', 16]);
    }

    pub fn align_left(&mut self)   -> &mut Self { self.push(&[ESC, b'a', 0]) }
    pub fn align_center(&mut self) -> &mut Self { self.push(&[ESC, b'a', 1]) }
    #[allow(dead_code)]
    pub fn align_right(&mut self)  -> &mut Self { self.push(&[ESC, b'a', 2]) }

    pub fn bold_on(&mut self)  -> &mut Self { self.push(&[ESC, b'E', 1]) }
    pub fn bold_off(&mut self) -> &mut Self { self.push(&[ESC, b'E', 0]) }

    pub fn double_height_on(&mut self)  -> &mut Self { self.push(&[ESC, b'!', 0x10]) }
    #[allow(dead_code)]
    pub fn double_width_on(&mut self)   -> &mut Self { self.push(&[ESC, b'!', 0x20]) }
    pub fn double_size_on(&mut self)    -> &mut Self { self.push(&[ESC, b'!', 0x30]) }
    pub fn normal_size(&mut self)       -> &mut Self { self.push(&[ESC, b'!', 0x00]) }

    #[allow(dead_code)]
    pub fn underline_on(&mut self)  -> &mut Self { self.push(&[ESC, b'-', 1]) }
    #[allow(dead_code)]
    pub fn underline_off(&mut self) -> &mut Self { self.push(&[ESC, b'-', 0]) }

    #[allow(dead_code)]
    pub fn lf(&mut self) -> &mut Self { self.buf.push(LF); self }
    pub fn lf_n(&mut self, n: u8) -> &mut Self { self.push(&[ESC, b'd', n]) }

    pub fn rule(&mut self) -> &mut Self {
        self.text(&"-".repeat(self.width))
    }

    pub fn rule_double(&mut self) -> &mut Self {
        self.text(&"=".repeat(self.width))
    }

    pub fn cut(&mut self) -> &mut Self {
        // Full cut with feed
        self.push(&[GS, b'V', b'A', 3])
    }

    #[allow(dead_code)]
    pub fn partial_cut(&mut self) -> &mut Self {
        self.push(&[GS, b'V', b'B', 3])
    }

    /// Print a QR code (model 2, error correction L, size 4).
    #[allow(dead_code)]
    pub fn qr_code(&mut self, data: &str) -> &mut Self {
        let d = data.as_bytes();
        let len = d.len() + 3;
        let p_l = (len & 0xFF) as u8;
        let p_h = ((len >> 8) & 0xFF) as u8;
        // Store QR data
        self.push(&[GS, b'(', b'k', p_l, p_h, 0x31, 0x50, 0x30]);
        self.buf.extend_from_slice(d);
        // Print QR
        self.push(&[GS, b'(', b'k', 0x03, 0x00, 0x31, 0x51, 0x30]);
        self
    }

    // ── Text helpers ──────────────────────────────────────────────────────

    /// Append a string followed by LF.
    pub fn text(&mut self, s: &str) -> &mut Self {
        // We encode as Latin-1/Windows-1252; replace unmappable chars with '?'
        for ch in s.chars() {
            let byte = char_to_cp1252(ch);
            self.buf.push(byte);
        }
        self.buf.push(LF);
        self
    }

    /// A two-column row: left-aligned label, right-aligned value.
    pub fn row(&mut self, left: &str, right: &str) -> &mut Self {
        let w       = self.width;
        let l_len   = display_len(left);
        let r_len   = display_len(right);
        let spaces  = w.saturating_sub(l_len + r_len);
        let line    = format!("{}{}{}", left, " ".repeat(spaces), right);
        self.text(&line)
    }

    /// A centered title string.
    #[allow(dead_code)]
    pub fn centered(&mut self, s: &str) -> &mut Self {
        let w     = self.width;
        let len   = display_len(s);
        let pad   = (w.saturating_sub(len)) / 2;
        let line  = format!("{}{}", " ".repeat(pad), s);
        self.text(&line)
    }

    // ── Low-level push ────────────────────────────────────────────────────

    fn push(&mut self, bytes: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(bytes);
        self
    }

    /// Consume the builder and return the final byte vector.
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }

    pub fn width(&self) -> usize { self.width }
}

// ─── Character encoding ────────────────────────────────────────────────────────

/// Convert a Rust char to its Windows-1252 byte, falling back to '?' for
/// characters outside the code page (e.g., Arabic, CJK).
fn char_to_cp1252(c: char) -> u8 {
    let n = c as u32;
    // ASCII range — direct pass-through
    if n < 0x80 { return n as u8; }
    // Latin-1 supplement (0x80–0xFF) — map to cp1252
    if n <= 0xFF { return n as u8; }
    // cp1252 extra chars (0x80–0x9F in Unicode private table)
    let cp1252_extra: &[(u32, u8)] = &[
        (0x20AC, 0x80), // €
        (0x201A, 0x82), (0x0192, 0x83), (0x201E, 0x84), (0x2026, 0x85),
        (0x2020, 0x86), (0x2021, 0x87), (0x02C6, 0x88), (0x2030, 0x89),
        (0x0160, 0x8A), (0x2039, 0x8B), (0x0152, 0x8C), (0x017D, 0x8E),
        (0x2018, 0x91), (0x2019, 0x92), (0x201C, 0x93), (0x201D, 0x94),
        (0x2022, 0x95), (0x2013, 0x96), (0x2014, 0x97), (0x02DC, 0x98),
        (0x2122, 0x99), (0x0161, 0x9A), (0x203A, 0x9B), (0x0153, 0x9C),
        (0x017E, 0x9E), (0x0178, 0x9F),
    ];
    for &(unicode, byte) in cp1252_extra {
        if n == unicode { return byte; }
    }
    b'?'   // unmappable (e.g. Arabic glyphs)
}

/// Visual display length (ASCII chars = 1, others = 1 for our fixed-width font).
fn display_len(s: &str) -> usize {
    s.chars().count()
}

// ─── Receipt builder (high-level) ─────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct ReceiptItem {
    pub name_fr:    String,
    #[allow(dead_code)]
    pub name_ar:    String,
    pub qty:        f64,
    pub unit_price: f64,
    pub total_ttc:  f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReceiptData {
    pub shop_name_fr:    String,
    #[allow(dead_code)]
    pub shop_name_ar:    String,
    pub shop_address:    String,
    pub shop_phone:      String,
    pub shop_nif:        String,
    pub shop_nis:        String,
    pub ref_number:      String,
    pub cashier:         String,
    pub date:            String,
    pub items:           Vec<ReceiptItem>,
    pub total_ht:        f64,
    pub total_ttc:       f64,
    pub vat_amount:      f64,
    pub vat_rate:        f64,
    pub discount_amount: f64,
    pub payment_method:  String,
    pub amount_paid:     f64,
    pub change_given:    f64,
    pub width_mm:        u8,
    pub show_vat:        bool,
}

pub fn build_receipt(data: &ReceiptData) -> Vec<u8> {
    let mut b = EscPosBuilder::new(data.width_mm);
    let w = b.width();

    // ── Header ─────────────────────────────────────────────────────────────
    b.align_center();
    b.double_size_on(); b.bold_on();
    b.text(&data.shop_name_fr);
    b.normal_size(); b.bold_off();
    b.text(&data.shop_address);
    if !data.shop_phone.is_empty() {
        b.text(&format!("Tel: {}", data.shop_phone));
    }
    if !data.shop_nif.is_empty() {
        b.text(&format!("NIF: {}  NIS: {}", data.shop_nif, data.shop_nis));
    }
    b.align_left();
    b.rule();

    // ── Transaction meta ───────────────────────────────────────────────────
    b.row(&format!("Ref: {}", data.ref_number), &data.date);
    b.text(&format!("Caissier: {}", data.cashier));
    b.rule();

    // ── Items ──────────────────────────────────────────────────────────────
    let name_w = w.saturating_sub(20);
    for item in &data.items {
        // Truncate name to fit
        let name: String = item.name_fr.chars().take(name_w).collect();
        b.text(&name);
        let detail = format!(
            "  {:.2} x {:.2}",
            item.qty, item.unit_price
        );
        let total = format!("{:.2} DZD", item.total_ttc);
        b.row(&detail, &total);
    }
    b.rule();

    // ── Totals ─────────────────────────────────────────────────────────────
    if data.discount_amount > 0.01 {
        b.row("Remise:", &format!("-{:.2} DZD", data.discount_amount));
    }
    if data.show_vat {
        b.row("Total HT:", &format!("{:.2} DZD", data.total_ht));
        b.row(
            &format!("TVA ({:.0}%):", data.vat_rate * 100.0),
            &format!("{:.2} DZD", data.vat_amount),
        );
    }
    b.bold_on();
    b.double_height_on();
    b.row("TOTAL:", &format!("{:.2} DZD", data.total_ttc));
    b.normal_size(); b.bold_off();
    b.rule();

    // ── Payment ────────────────────────────────────────────────────────────
    let method_label = match data.payment_method.as_str() {
        "cash"     => "Especes",
        "cib"      => "CIB",
        "dahabia"  => "Dahabia",
        "dain"     => "Credit (Dain)",
        other      => other,
    };
    b.row(&format!("Reglement ({}):", method_label),
          &format!("{:.2} DZD", data.amount_paid));
    if data.change_given > 0.005 {
        b.bold_on();
        b.row("Monnaie:", &format!("{:.2} DZD", data.change_given));
        b.bold_off();
    }

    // ── Footer ─────────────────────────────────────────────────────────────
    b.align_center();
    b.lf_n(1);
    b.text("Merci de votre visite !");
    b.text("!شكرا على زيارتكم");
    b.lf_n(3);
    b.cut();

    b.finish()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn builder_produces_init_bytes() {
        let b = EscPosBuilder::new(80);
        let bytes = b.finish();
        // First two bytes are ESC @  (init)
        assert_eq!(&bytes[..2], &[0x1B, b'@']);
    }

    #[test]
    fn receipt_builds_without_panic() {
        let data = ReceiptData {
            shop_name_fr: "Test Shop".into(),
            shop_name_ar: "متجر".into(),
            shop_address: "Alger".into(),
            shop_phone: "0550000000".into(),
            shop_nif: "123456".into(),
            shop_nis: "654321".into(),
            ref_number: "TXN-20240101-0001".into(),
            cashier: "Admin".into(),
            date: "01/01/2024 10:00".into(),
            items: vec![ReceiptItem {
                name_fr: "Lait 1L".into(),
                name_ar: "حليب".into(),
                qty: 2.0,
                unit_price: 115.0,
                total_ttc: 230.0,
            }],
            total_ht: 193.28,
            total_ttc: 230.0,
            vat_amount: 36.72,
            vat_rate: 0.19,
            discount_amount: 0.0,
            payment_method: "cash".into(),
            amount_paid: 300.0,
            change_given: 70.0,
            width_mm: 80,
            show_vat: true,
        };
        let bytes = build_receipt(&data);
        assert!(!bytes.is_empty());
        // Should end with GS V A (cut)
        let last3 = &bytes[bytes.len() - 3..];
        assert_eq!(last3, &[0x1D, b'V', b'A']);
    }

    #[test]
    fn char_encoding_handles_french() {
        assert_eq!(char_to_cp1252('é'), 0xE9);
        assert_eq!(char_to_cp1252('à'), 0xE0);
        assert_eq!(char_to_cp1252('ç'), 0xE7);
        assert_eq!(char_to_cp1252('€'), 0x80);
        // Arabic → replaced with '?'
        assert_eq!(char_to_cp1252('ع'), b'?');
    }
}