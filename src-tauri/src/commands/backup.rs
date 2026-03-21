// src-tauri/src/commands/backup.rs
//! Database backup and Excel export commands.
//!
//! Backup strategy:
//!   • SQLite "hot backup" via the `VACUUM INTO` pragma — produces a clean,
//!     defragmented copy without locking the main database.
//!   • Files are timestamped: superpos_backup_YYYYMMDD_HHMMSS.db
//!   • Max 30 backups retained (oldest auto-deleted).
//!
//! Excel export uses `rust_xlsxwriter` to build a proper .xlsx report
//! (formatted headers, number formats, auto-column widths) without any
//! external runtime dependency.

use chrono::Utc;
use rusqlite::params;
use rust_xlsxwriter::{
    Format, FormatAlign, FormatBorder, Workbook, XlsxError,
    Color,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{command, AppHandle, Manager, State};

use crate::AppState;

const MAX_BACKUPS: usize = 30;

// ─── Backup ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct BackupResult {
    pub path:       String,
    pub size_kb:    u64,
    pub created_at: String,
}

/// Create a timestamped backup of the SQLite database.
#[command]
pub async fn cmd_create_backup(
    app:   AppHandle,
    state: State<'_, AppState>,
) -> Result<BackupResult, String> {
    let app_dir     = resolve_app_dir(&app)?;
    let backup_dir  = app_dir.join("backups");
    std::fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Cannot create backup dir: {e}"))?;

    let ts       = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("superpos_backup_{ts}.db");
    let dest     = backup_dir.join(&filename);

    // VACUUM INTO produces a fresh, unlocked copy atomically.
    {
        let db   = state.db.lock().unwrap();
        let conn = &db.0;
        conn.execute(
            &format!("VACUUM INTO '{}'", dest.to_string_lossy()),
            [],
        ).map_err(|e| format!("Backup failed: {e}"))?;
    }

    let size_kb = std::fs::metadata(&dest)
        .map(|m| m.len() / 1024)
        .unwrap_or(0);

    // Prune old backups
    prune_backups(&backup_dir);

    Ok(BackupResult {
        path:       dest.to_string_lossy().to_string(),
        size_kb,
        created_at: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })
}

/// List all existing backups (newest first).
#[command]
pub async fn cmd_list_backups(app: AppHandle) -> Result<Vec<BackupResult>, String> {
    let app_dir    = resolve_app_dir(&app)?;
    let backup_dir = app_dir.join("backups");
    if !backup_dir.exists() { return Ok(vec![]); }

    let mut entries: Vec<_> = std::fs::read_dir(&backup_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name().to_string_lossy().ends_with(".db")
        })
        .collect();

    // Sort newest first
    entries.sort_by_key(|e| {
        e.metadata().and_then(|m| m.modified()).ok()
    });
    entries.reverse();

    Ok(entries.iter().map(|e| {
        let path    = e.path();
        let size_kb = e.metadata().map(|m| m.len() / 1024).unwrap_or(0);
        BackupResult {
            path:       path.to_string_lossy().to_string(),
            size_kb,
            created_at: String::new(),
        }
    }).collect())
}

fn prune_backups(dir: &Path) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".db"))
        .map(|e| e.path())
        .collect();

    entries.sort();   // oldest first (timestamp in filename)

    while entries.len() > MAX_BACKUPS {
        if let Some(oldest) = entries.first() {
            let _ = std::fs::remove_file(oldest);
            entries.remove(0);
        }
    }
}

// ─── Excel export ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExportReportRequest {
    pub date_from: String,
    pub date_to:   String,
}

#[derive(Debug, Serialize)]
pub struct ExportResult {
    pub path:     String,
    pub rows:     usize,
}

/// Export a date-range sales report to a formatted .xlsx file.
#[command]
pub async fn cmd_export_sales_excel(
    app:     AppHandle,
    state:   State<'_, AppState>,
    request: ExportReportRequest,
) -> Result<ExportResult, String> {
    let app_dir   = resolve_app_dir(&app)?;
    let export_dir = app_dir.join("exports");
    std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;

    let ts       = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("rapport_ventes_{ts}.xlsx");
    let dest     = export_dir.join(&filename);

    let rows = write_excel(
        &state,
        &request.date_from,
        &request.date_to,
        &dest,
    ).map_err(|e| format!("Excel error: {e}"))?;

    Ok(ExportResult {
        path: dest.to_string_lossy().to_string(),
        rows,
    })
}

fn write_excel(
    state:     &State<'_, AppState>,
    date_from: &str,
    date_to:   &str,
    dest:      &Path,
) -> Result<usize, XlsxError> {
    let mut workbook = Workbook::new();

    // ── Formats ────────────────────────────────────────────────────────────
    let header_fmt = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x1a1050))
        .set_font_color(Color::White)
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);

    let money_fmt = Format::new()
        .set_num_format("#,##0.00\" DZD\"")
        .set_border(FormatBorder::Thin);

    let date_fmt = Format::new()
        .set_num_format("dd/mm/yyyy hh:mm")
        .set_border(FormatBorder::Thin);

    let normal_fmt = Format::new()
        .set_border(FormatBorder::Thin);

    let total_fmt = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xECEAFD))
        .set_num_format("#,##0.00\" DZD\"")
        .set_border(FormatBorder::Thin);

    // ── Sheet 1: Transactions ──────────────────────────────────────────────
    let ws = workbook.add_worksheet();
    ws.set_name("Ventes")?;

    // Headers
    let headers = [
        "Référence", "Date", "Client", "Mode Paiement",
        "Total HT (DZD)", "TVA (DZD)", "Total TTC (DZD)",
        "Remise (DZD)", "Payé (DZD)", "Rendu (DZD)", "Caissier",
    ];
    for (col, h) in headers.iter().enumerate() {
        ws.write_with_format(0, col as u16, *h, &header_fmt)?;
    }

    // Data rows
    let rows_data: Vec<(String, String, Option<String>, String, f64, f64, f64, f64, f64, f64, String)>;
    {
        let db   = state.db.lock().unwrap();
        let conn = &db.0;
        let mut stmt = conn.prepare(
            "SELECT t.ref_number, t.created_at,
                    c.name AS customer,
                    t.payment_method,
                    t.total_ht, (t.total_ttc - t.total_ht) AS vat,
                    t.total_ttc, t.discount_amount,
                    t.amount_paid, t.change_given, t.cashier_name
             FROM transactions t
             LEFT JOIN customers c ON c.id = t.customer_id
             WHERE DATE(t.created_at) BETWEEN ?1 AND ?2
             ORDER BY t.created_at"
        ).map_err(|e| XlsxError::CustomError(e.to_string()))?;

        rows_data = stmt.query_map(params![date_from, date_to], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, f64>(6)?,
                r.get::<_, f64>(7)?,
                r.get::<_, f64>(8)?,
                r.get::<_, f64>(9)?,
                r.get::<_, String>(10)?,
            ))
        })
        .map_err(|e| XlsxError::CustomError(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();
    }

    let row_count = rows_data.len();

    for (i, row) in rows_data.iter().enumerate() {
        let r = (i + 1) as u32;
        ws.write_with_format(r, 0,  &row.0,                    &normal_fmt)?;
        ws.write_with_format(r, 1,  row.1.replace('T', " ").replace('Z', ""), &normal_fmt)?;
        ws.write_with_format(r, 2,  row.2.as_deref().unwrap_or("—"), &normal_fmt)?;
        ws.write_with_format(r, 3,  &row.3,                    &normal_fmt)?;
        ws.write_with_format(r, 4,  row.4,                     &money_fmt)?;
        ws.write_with_format(r, 5,  row.5,                     &money_fmt)?;
        ws.write_with_format(r, 6,  row.6,                     &money_fmt)?;
        ws.write_with_format(r, 7,  row.7,                     &money_fmt)?;
        ws.write_with_format(r, 8,  row.8,                     &money_fmt)?;
        ws.write_with_format(r, 9,  row.9,                     &money_fmt)?;
        ws.write_with_format(r, 10, &row.10,                   &normal_fmt)?;
    }

    // Totals row
    if row_count > 0 {
        let tr = (row_count + 1) as u32;
        ws.write_with_format(tr, 0, "TOTAL", &total_fmt)?;
        ws.write_with_format(tr, 1, "", &total_fmt)?;
        ws.write_with_format(tr, 2, "", &total_fmt)?;
        ws.write_with_format(tr, 3, "", &total_fmt)?;
        // Sum formulas
        for col in 4u16..=9 {
            let formula = format!(
                "=SUM({0}2:{0}{1})",
                col_letter(col),
                row_count + 1
            );
            ws.write_formula_with_format(tr, col, &formula, &total_fmt)?;
        }
        ws.write_with_format(tr, 10, "", &total_fmt)?;
    }

    // Column widths
    let col_widths = [18.0, 18.0, 20.0, 14.0, 14.0, 12.0, 14.0, 12.0, 12.0, 12.0, 14.0];
    for (col, w) in col_widths.iter().enumerate() {
        ws.set_column_width(col as u16, *w)?;
    }

    // ── Sheet 2: Top products by revenue ──────────────────────────────────
    let ws2 = workbook.add_worksheet();
    ws2.set_name("Top Produits")?;

    let prod_headers = ["Produit", "Qté vendue", "CA TTC (DZD)"];
    for (col, h) in prod_headers.iter().enumerate() {
        ws2.write_with_format(0, col as u16, *h, &header_fmt)?;
    }

    let prod_rows: Vec<(String, f64, f64)>;
    {
        let db   = state.db.lock().unwrap();
        let conn = &db.0;
        let mut stmt = conn.prepare(
            "SELECT p.name_fr,
                    SUM(ti.quantity) AS total_qty,
                    SUM(ti.quantity * ti.unit_price * (1 + ti.vat_rate)
                        * (1 - ti.discount_pct / 100.0)) AS revenue
             FROM transaction_items ti
             JOIN products p ON p.id = ti.product_id
             JOIN transactions t ON t.id = ti.transaction_id
             WHERE DATE(t.created_at) BETWEEN ?1 AND ?2
             GROUP BY p.id
             ORDER BY revenue DESC
             LIMIT 50"
        ).map_err(|e| XlsxError::CustomError(e.to_string()))?;

        prod_rows = stmt.query_map(params![date_from, date_to], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?, r.get::<_, f64>(2)?))
        })
        .map_err(|e| XlsxError::CustomError(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect();
    }

    for (i, (name, qty, rev)) in prod_rows.iter().enumerate() {
        let r = (i + 1) as u32;
        ws2.write_with_format(r, 0, name,  &normal_fmt)?;
        ws2.write_with_format(r, 1, *qty,  &normal_fmt)?;
        ws2.write_with_format(r, 2, *rev,  &money_fmt)?;
    }
    ws2.set_column_width(0, 28.0)?;
    ws2.set_column_width(1, 14.0)?;
    ws2.set_column_width(2, 16.0)?;

    workbook.save(dest)?;
    Ok(row_count)
}

fn col_letter(col: u16) -> &'static str {
    // Only needed for columns 4–9 (E–J)
    match col {
        0 => "A", 1 => "B", 2 => "C", 3 => "D",
        4 => "E", 5 => "F", 6 => "G", 7 => "H",
        8 => "I", 9 => "J", 10 => "K", _ => "Z",
    }
}

fn resolve_app_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}