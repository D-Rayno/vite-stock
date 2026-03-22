// src-tauri/src/utils/pdf.rs
//! A4 PDF generation for SuperPOS documents.
//!
//! Uses `printpdf` 0.6.x — all shapes are drawn with the `Line` primitive.
//! (`Rect` and `path::PaintMode` were added in later versions; we use a
//! closed `Line { has_fill: true }` instead, which works identically.)

use printpdf::{
    BuiltinFont, Color, IndirectFontRef, Line, Mm, PdfDocument,
    PdfDocumentReference, PdfLayerIndex, PdfLayerReference, PdfPageIndex,
    Point, Rgb,
};
use std::path::PathBuf;

// ── Page geometry ──────────────────────────────────────────────────────────────

const A4_W: f64 = 210.0;
const A4_H: f64 = 297.0;
const MARGIN_L: f64 = 20.0;
const MARGIN_R: f64 = 20.0;
const MARGIN_TOP: f64 = 20.0;
const MARGIN_BOT: f64 = 20.0;

pub const CONTENT_W: f64 = A4_W - MARGIN_L - MARGIN_R;
pub const LH_SMALL: f64 = 5.0;
pub const LH_BODY: f64  = 6.0;
pub const LH_TITLE: f64 = 8.0;

// ── Colour helpers ─────────────────────────────────────────────────────────────

fn rgb(r: f64, g: f64, b: f64) -> Color { Color::Rgb(Rgb::new(r, g, b, None)) }

pub fn black()      -> Color { rgb(0.0,  0.0,  0.0)  }
pub fn gray()       -> Color { rgb(0.45, 0.45, 0.45) }
pub fn light_gray() -> Color { rgb(0.88, 0.88, 0.88) }
pub fn brand()      -> Color { rgb(0.20, 0.13, 0.78) }
pub fn red()        -> Color { rgb(0.82, 0.13, 0.15) }
pub fn green()      -> Color { rgb(0.09, 0.55, 0.24) }
pub fn amber()      -> Color { rgb(0.65, 0.40, 0.00) }
pub fn white()      -> Color { rgb(1.0,  1.0,  1.0)  }

// ── Internal: draw a filled rectangle via a closed Line ───────────────────────

fn draw_filled_rect(
    layer: &PdfLayerReference,
    x1: f64, y1: f64, x2: f64, y2: f64,
    color: Color,
) {
    layer.set_fill_color(color);
    layer.set_outline_thickness(0.0);
    layer.add_line(Line {
        points: vec![
            (Point::new(Mm(x1), Mm(y1)), false),
            (Point::new(Mm(x2), Mm(y1)), false),
            (Point::new(Mm(x2), Mm(y2)), false),
            (Point::new(Mm(x1), Mm(y2)), false),
        ],
        is_closed:        true,
        has_fill:         true,
        has_stroke:       false,
        is_clipping_path: false,
    });
}

// ── PdfCanvas ─────────────────────────────────────────────────────────────────

pub struct PdfCanvas {
    doc:        PdfDocumentReference,
    pages:      Vec<(PdfPageIndex, PdfLayerIndex)>,
    pub font:   IndirectFontRef,
    pub font_b: IndirectFontRef,
    pub font_i: IndirectFontRef,
    pub cursor_y: f64,
}

impl PdfCanvas {
    pub fn new(title: &str) -> Self {
        let (doc, p, l) = PdfDocument::new(title, Mm(A4_W), Mm(A4_H), "Content");
        let font   = doc.add_builtin_font(BuiltinFont::Helvetica).unwrap();
        let font_b = doc.add_builtin_font(BuiltinFont::HelveticaBold).unwrap();
        let font_i = doc.add_builtin_font(BuiltinFont::HelveticaOblique).unwrap();
        Self {
            doc,
            pages: vec![(p, l)],
            font,
            font_b,
            font_i,
            cursor_y: A4_H - MARGIN_TOP,
        }
    }

    // ── Page management ───────────────────────────────────────────────────────

    pub fn current_layer(&self) -> PdfLayerReference {
        let (page_idx, layer_idx) = *self.pages.last().unwrap();
        self.doc.get_page(page_idx).get_layer(layer_idx)
    }

    pub fn new_page(&mut self) {
        let (p, l) = self.doc.add_page(Mm(A4_W), Mm(A4_H), "Content");
        self.pages.push((p, l));
        self.cursor_y = A4_H - MARGIN_TOP;
    }

    pub fn ensure_space(&mut self, needed_mm: f64) {
        if self.cursor_y - needed_mm < MARGIN_BOT {
            self.new_page();
        }
    }

    // ── Primitives ────────────────────────────────────────────────────────────

    pub fn hline(&mut self, thickness: f64, color: Color) {
        let layer = self.current_layer();
        layer.set_outline_color(color);
        layer.set_outline_thickness(thickness);
        layer.add_line(Line {
            points: vec![
                (Point::new(Mm(MARGIN_L), Mm(self.cursor_y)), false),
                (Point::new(Mm(A4_W - MARGIN_R), Mm(self.cursor_y)), false),
            ],
            is_closed:        false,
            has_fill:         false,
            has_stroke:       true,
            is_clipping_path: false,
        });
    }

    /// Draw a filled band across the full content width.
    /// `y`  = top of the band (in canvas coords, top = 0)
    /// `h`  = height of the band in mm
    pub fn filled_rect(&mut self, y: f64, h: f64, color: Color) {
        let layer = self.current_layer();
        draw_filled_rect(&layer, MARGIN_L, y - h, A4_W - MARGIN_R, y, color);
    }

    pub fn gap(&mut self, mm: f64) { self.cursor_y -= mm; }

    // ── Text ─────────────────────────────────────────────────────────────────

    pub fn text(&mut self, s: &str, size: f64, x: f64, color: Color, font: &IndirectFontRef) {
        let layer = self.current_layer();
        layer.set_fill_color(color);
        layer.use_text(s, size, Mm(x), Mm(self.cursor_y), font);
    }

    pub fn kv_row(&mut self, key: &str, value: &str, size: f64) {
        let layer  = self.current_layer();
        let bold   = self.font_b.clone();
        let normal = self.font.clone();
        layer.set_fill_color(black());
        layer.use_text(key, size, Mm(MARGIN_L), Mm(self.cursor_y), &bold);
        let approx_w = value.len() as f64 * size * 0.55 * 0.352_778;
        let x_val = (A4_W - MARGIN_R - approx_w).max(MARGIN_L + 60.0);
        layer.use_text(value, size, Mm(x_val), Mm(self.cursor_y), &normal);
        self.cursor_y -= LH_BODY;
    }

    // ── Structured blocks ─────────────────────────────────────────────────────

    pub fn header(
        &mut self,
        shop_name:  &str,
        address:    &str,
        phone:      &str,
        nif:        &str,
        nis:        &str,
        doc_title:  &str,
        doc_ref:    &str,
        date:       &str,
    ) {
        // Brand bar
        self.filled_rect(self.cursor_y + 2.0, 14.0, brand());
        let layer = self.current_layer();
        let fb    = self.font_b.clone();
        let fn_   = self.font.clone();
        layer.set_fill_color(white());
        layer.use_text(shop_name, 14.0, Mm(MARGIN_L), Mm(self.cursor_y - 2.0), &fb);
        let right_x = A4_W - MARGIN_R - 45.0;
        layer.use_text(doc_title, 11.0, Mm(right_x), Mm(self.cursor_y - 2.0), &fb);
        self.cursor_y -= 8.0;

        layer.set_fill_color(gray());
        layer.use_text(address, 8.5, Mm(MARGIN_L), Mm(self.cursor_y), &fn_);
        if !phone.is_empty() {
            layer.use_text(&format!("Tel: {phone}"), 8.5, Mm(right_x), Mm(self.cursor_y), &fn_);
        }
        self.cursor_y -= 5.0;

        if !nif.is_empty() {
            layer.use_text(&format!("NIF: {nif}  NIS: {nis}"), 8.0, Mm(MARGIN_L), Mm(self.cursor_y), &fn_);
        }
        layer.use_text(&format!("Ref: {doc_ref}  |  {date}"), 8.0, Mm(right_x), Mm(self.cursor_y), &fn_);
        self.cursor_y -= 5.0;

        self.hline(0.8, brand());
        self.cursor_y -= 5.0;
    }

    pub fn section_title(&mut self, title: &str) {
        self.ensure_space(12.0);
        let layer = self.current_layer();
        let fb    = self.font_b.clone();
        layer.set_fill_color(brand());
        layer.use_text(title, 11.0, Mm(MARGIN_L), Mm(self.cursor_y), &fb);
        self.cursor_y -= 1.5;
        self.hline(0.4, brand());
        self.cursor_y -= 5.0;
    }

    /// `cols`: (header_label, width_mm, alignment)  alignment: 0=left 1=right
    pub fn table(
        &mut self,
        cols: &[(&str, f64, u8)],
        rows: &[Vec<(String, Option<Color>)>],
    ) {
        let row_h    = 6.5_f64;
        let header_h = 7.0_f64;

        self.ensure_space(header_h + row_h);

        // Header background band
        self.filled_rect(self.cursor_y + 1.0, header_h, brand());

        let layer = self.current_layer();
        let fb    = self.font_b.clone();
        let fn_   = self.font.clone();

        let mut cx = MARGIN_L + 1.0;
        for (label, w, align) in cols {
            layer.set_fill_color(white());
            let x = if *align == 1 {
                let approx = label.len() as f64 * 8.5 * 0.55 * 0.352_778;
                (cx + w - approx - 1.0).max(cx)
            } else {
                cx
            };
            layer.use_text(label, 8.5, Mm(x), Mm(self.cursor_y), &fb);
            cx += w;
        }
        self.cursor_y -= header_h;

        // Data rows
        for (i, row) in rows.iter().enumerate() {
            self.ensure_space(row_h + 2.0);
            if i % 2 == 1 {
                self.filled_rect(self.cursor_y + 1.0, row_h, light_gray());
            }
            let layer = self.current_layer();
            let mut cx = MARGIN_L + 1.0;
            for (j, (cell_text, cell_color)) in row.iter().enumerate() {
                if j >= cols.len() { break; }
                let (_, w, align) = &cols[j];
                let color = cell_color.clone().unwrap_or_else(black);
                layer.set_fill_color(color);
                let x = if *align == 1 {
                    let approx = cell_text.len() as f64 * 8.0 * 0.55 * 0.352_778;
                    (cx + w - approx - 1.0).max(cx)
                } else {
                    cx
                };
                layer.use_text(cell_text.as_str(), 8.0, Mm(x), Mm(self.cursor_y), &fn_);
                cx += w;
            }
            self.cursor_y -= row_h;
        }

        self.hline(0.4, gray());
        self.cursor_y -= 3.0;
    }

    pub fn add_footers(&mut self, generated_at: &str) {
        let total = self.pages.len();
        for (i, (page_idx, layer_idx)) in self.pages.iter().enumerate() {
            let layer = self.doc.get_page(*page_idx).get_layer(*layer_idx);
            let fn_   = self.font.clone();
            layer.set_fill_color(gray());
            layer.use_text(
                &format!("Generé le {generated_at}  —  SuperPOS"),
                7.5, Mm(MARGIN_L), Mm(MARGIN_BOT - 5.0), &fn_,
            );
            let page_label = format!("Page {} / {}", i + 1, total);
            let approx_w   = page_label.len() as f64 * 7.5 * 0.55 * 0.352_778;
            layer.use_text(
                &page_label,
                7.5, Mm(A4_W - MARGIN_R - approx_w), Mm(MARGIN_BOT - 5.0), &fn_,
            );
        }
    }

    pub fn save(self) -> Result<Vec<u8>, String> {
        self.doc.save_to_bytes().map_err(|e| e.to_string())
    }
}

// ── Document builders ─────────────────────────────────────────────────────────

pub struct ShopInfo<'a> {
    pub name:    &'a str,
    pub address: &'a str,
    pub phone:   &'a str,
    pub nif:     &'a str,
    pub nis:     &'a str,
}

pub struct DainEntryPdf {
    pub date:          String,
    pub entry_type:    String,
    pub amount:        f64,
    pub notes:         String,
    pub balance_after: f64,
}

pub struct DainStatementData<'a> {
    pub shop:           ShopInfo<'a>,
    pub customer_name:  &'a str,
    pub customer_phone: &'a str,
    pub balance:        f64,
    pub credit_limit:   f64,
    pub entries:        &'a [DainEntryPdf],
    pub generated_at:   &'a str,
    pub doc_ref:        &'a str,
}

pub fn build_dain_statement(data: &DainStatementData<'_>) -> Result<Vec<u8>, String> {
    let title = format!("Releve Dain — {}", data.customer_name);
    let mut c = PdfCanvas::new(&title);

    c.header(
        data.shop.name, data.shop.address, data.shop.phone,
        data.shop.nif, data.shop.nis,
        "RELEVE DE COMPTE CLIENT",
        data.doc_ref, data.generated_at,
    );

    c.section_title("Informations client");
    {
        let layer = c.current_layer();
        let fb    = c.font_b.clone();
        let fn_   = c.font.clone();
        layer.set_fill_color(black());
        layer.use_text(data.customer_name, 12.0, Mm(MARGIN_L), Mm(c.cursor_y), &fb);
        layer.use_text(
            &format!("Tel: {}", data.customer_phone),
            10.0, Mm(MARGIN_L + 80.0), Mm(c.cursor_y), &fn_,
        );
    }
    c.cursor_y -= LH_BODY;

    // Balance summary box
    c.ensure_space(20.0);
    let balance_color = if data.balance > 0.0 { red() } else { green() };
    c.filled_rect(c.cursor_y + 1.0, 14.0, light_gray());
    {
        let layer = c.current_layer();
        let fb    = c.font_b.clone();
        let fn_   = c.font.clone();
        layer.set_fill_color(gray());
        layer.use_text("Solde actuel", 9.0, Mm(MARGIN_L + 4.0), Mm(c.cursor_y - 1.0), &fn_);
        layer.set_fill_color(balance_color);
        let sign  = if data.balance > 0.0 { "DU " } else { "CREDITEUR " };
        let label = format!("{sign}{:.2} DZD", data.balance.abs());
        layer.use_text(&label, 14.0, Mm(MARGIN_L + 4.0), Mm(c.cursor_y - 8.0), &fb);
        if data.credit_limit > 0.0 {
            layer.set_fill_color(gray());
            layer.use_text(
                &format!("Limite credit: {:.2} DZD", data.credit_limit),
                8.5, Mm(MARGIN_L + 90.0), Mm(c.cursor_y - 5.0), &fn_,
            );
        }
    }
    c.cursor_y -= 17.0;
    c.gap(3.0);

    c.section_title("Historique des transactions");

    let cols: &[(&str, f64, u8)] = &[
        ("Date",          38.0, 0),
        ("Type",          32.0, 0),
        ("Montant (DZD)", 40.0, 1),
        ("Solde (DZD)",   40.0, 1),
        ("Notes",         20.0, 0),
    ];

    let rows: Vec<Vec<(String, Option<Color>)>> = data.entries.iter().map(|e| {
        let type_color = if e.entry_type == "Debit" { Some(red()) } else { Some(green()) };
        let amount_str = if e.entry_type == "Debit" {
            format!("+ {:.2}", e.amount)
        } else {
            format!("- {:.2}", e.amount)
        };
        vec![
            (e.date.clone(),                          None),
            (e.entry_type.clone(),                    type_color),
            (amount_str,                              None),
            (format!("{:.2}", e.balance_after),       None),
            (e.notes.chars().take(18).collect(),      None),
        ]
    }).collect();

    c.table(cols, &rows);

    if data.entries.is_empty() {
        let fn_   = c.font.clone();
        let layer = c.current_layer();
        layer.set_fill_color(gray());
        layer.use_text("Aucune transaction enregistree.", 9.0, Mm(MARGIN_L), Mm(c.cursor_y), &fn_);
        c.cursor_y -= LH_BODY;
    }

    c.add_footers(data.generated_at);
    c.save()
}

// ── Stock report ───────────────────────────────────────────────────────────────

pub struct StockReportRow {
    pub product_name: String,
    pub gtin:         String,
    pub category:     String,
    pub quantity:     f64,
    pub unit:         String,
    pub expiry_date:  String,
    pub days_left:    Option<i64>,
    pub cost_price:   Option<f64>,
}

pub struct StockReportData<'a> {
    pub shop:         ShopInfo<'a>,
    pub rows:         &'a [StockReportRow],
    pub generated_at: &'a str,
    pub warn_only:    bool,
}

pub fn build_stock_report(data: &StockReportData<'_>) -> Result<Vec<u8>, String> {
    let doc_title = if data.warn_only { "RAPPORT ALERTES STOCK" } else { "RAPPORT INVENTAIRE COMPLET" };
    let mut c     = PdfCanvas::new(doc_title);
    let doc_ref   = format!("STK-{}", data.generated_at.split(' ').next().unwrap_or("").replace('-', ""));

    c.header(
        data.shop.name, data.shop.address, data.shop.phone,
        data.shop.nif, data.shop.nis,
        doc_title, &doc_ref, data.generated_at,
    );

    let expired  = data.rows.iter().filter(|r| r.days_left.map(|d| d < 0).unwrap_or(false)).count();
    let critical = data.rows.iter().filter(|r| r.days_left.map(|d| d >= 0 && d <= 7).unwrap_or(false)).count();
    let warning  = data.rows.iter().filter(|r| r.days_left.map(|d| d > 7 && d <= 30).unwrap_or(false)).count();

    c.section_title("Resume");
    c.kv_row("Total lignes :", &data.rows.len().to_string(), 9.0);
    if expired  > 0 { c.kv_row("Expires :",          &expired.to_string(),  9.0); }
    if critical > 0 { c.kv_row("Critiques (<=7j) :", &critical.to_string(), 9.0); }
    if warning  > 0 { c.kv_row("Attention (<=30j):", &warning.to_string(),  9.0); }
    c.gap(3.0);

    c.section_title("Detail des lots");

    let cols: &[(&str, f64, u8)] = &[
        ("Produit",      55.0, 0),
        ("Categorie",    28.0, 0),
        ("Qte",          18.0, 1),
        ("Unite",        14.0, 0),
        ("Expiration",   28.0, 0),
        ("Statut",       21.0, 0),
        ("Cout HT DZD",  26.0, 1),
    ];

    let rows: Vec<Vec<(String, Option<Color>)>> = data.rows.iter().map(|r| {
        let (status_str, status_color): (String, Option<Color>) = match r.days_left {
            None              => ("—".to_string(), None),
            Some(d) if d < 0  => ("Expire".to_string(),      Some(red())),
            Some(d) if d <= 7 => (format!("{}j !", d),       Some(red())),
            Some(d) if d <= 30 => (format!("{}j", d),        Some(amber())),
            Some(d)            => (format!("{}j", d),        Some(green())),
        };
        let cost_str = r.cost_price.map(|v| format!("{:.2}", v)).unwrap_or_else(|| "—".to_string());
        let qty_str  = if r.quantity % 1.0 == 0.0 { format!("{:.0}", r.quantity) } else { format!("{:.2}", r.quantity) };
        vec![
            (r.product_name.chars().take(26).collect(), None),
            (r.category.chars().take(14).collect(),     None),
            (qty_str,                                   None),
            (r.unit.clone(),                            None),
            (if r.expiry_date.is_empty() { "—".to_string() } else { r.expiry_date.clone() }, None),
            (status_str,                                status_color),
            (cost_str,                                  None),
        ]
    }).collect();

    c.table(cols, &rows);
    c.add_footers(data.generated_at);
    c.save()
}

// ── Utility ───────────────────────────────────────────────────────────────────

pub fn write_pdf_to_file(bytes: Vec<u8>, dir: &std::path::Path, name: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}