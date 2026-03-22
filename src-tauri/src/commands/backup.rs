// src-tauri/src/commands/backup.rs
//! Database backup and Excel export commands.
//!
//! Three commands are exposed to the frontend:
//!   `cmd_create_backup`      — VACUUM INTO snapshot → backups/superpos_manual_<ts>.db
//!   `cmd_list_backups`       — list all .db files in the backups directory
//!   `cmd_export_sales_excel` — export transactions table to .xlsx via rust_xlsxwriter

use chrono::Utc;
use rusqlite::params;
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook};
use serde::Serialize;
use tauri::{command, AppHandle, Manager, State};

use crate::AppState;

// ─── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct BackupInfo {
    pub filename:    String,
    pub size_bytes:  u64,
    pub created_at:  String,
    pub path:        String,
}

// ─── cmd_create_backup ────────────────────────────────────────────────────────

/// Perform a live `VACUUM INTO` backup of the SQLite database.
/// Returns metadata about the newly created file.
#[command]
pub async fn cmd_create_backup(
    app:   AppHandle,
    state: State<'_, AppState>,
) -> Result<BackupInfo, String> {
    let app_dir    = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let backup_dir = app_dir.join("backups");
    std::fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;

    let ts       = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("superpos_manual_{ts}.db");
    let dest     = backup_dir.join(&filename);

    // Hold the lock only for the VACUUM INTO call.
    {
        let db         = state.db.lock().unwrap();
        let vacuum_sql = format!("VACUUM INTO '{}'", dest.to_string_lossy());
        db.0.execute_batch(&vacuum_sql)
            .map_err(|e| format!("Sauvegarde échouée : {e}"))?;
    }

    let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);

    Ok(BackupInfo {
        filename,
        size_bytes: size,
        created_at: Utc::now().to_rfc3339(),
        path:       dest.to_string_lossy().to_string(),
    })
}

// ─── cmd_list_backups ─────────────────────────────────────────────────────────

/// Return all .db files in the backups directory, newest first.
#[command]
pub async fn cmd_list_backups(app: AppHandle) -> Result<Vec<BackupInfo>, String> {
    let app_dir    = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let backup_dir = app_dir.join("backups");

    if !backup_dir.exists() {
        return Ok(vec![]);
    }

    let mut backups: Vec<BackupInfo> = std::fs::read_dir(&backup_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".db"))
        .filter_map(|e| {
            let meta     = e.metadata().ok()?;
            let filename = e.file_name().to_string_lossy().to_string();
            let modified = meta.modified().ok()?;
            let dt: chrono::DateTime<Utc> = modified.into();
            Some(BackupInfo {
                filename,
                size_bytes: meta.len(),
                created_at: dt.to_rfc3339(),
                path:       e.path().to_string_lossy().to_string(),
            })
        })
        .collect();

    // Newest first
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(backups)
}

// ─── cmd_export_sales_excel ───────────────────────────────────────────────────

/// Export transactions in a date range to an Excel workbook.
/// Returns the absolute path of the written .xlsx file.
#[command]
pub async fn cmd_export_sales_excel(
    app:       AppHandle,
    state:     State<'_, AppState>,
    date_from: String,
    date_to:   String,
) -> Result<String, String> {
    // ── 1. Fetch data under a short DB lock ──────────────────────────────────
    let (rows, totals) = {
        let db   = state.db.lock().unwrap();
        let conn = &db.0;

        let mut stmt = conn.prepare(
            "SELECT t.ref_number,
                    DATE(t.created_at)          AS sale_date,
                    t.cashier_name,
                    COALESCE(c.name, '—')       AS customer,
                    t.payment_method,
                    t.total_ht,
                    t.total_ttc,
                    t.discount_amount,
                    t.amount_paid,
                    t.change_given,
                    CASE WHEN t.is_voided = 1 THEN 'Annulé' ELSE 'Validé' END AS statut
             FROM transactions t
             LEFT JOIN customers c ON c.id = t.customer_id
             WHERE DATE(t.created_at) BETWEEN ?1 AND ?2
             ORDER BY t.created_at",
        ).map_err(|e| e.to_string())?;

        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, String, String, String, f64, f64, f64, f64, f64, String)> =
            stmt.query_map(params![date_from, date_to], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, f64>(5)?,
                    r.get::<_, f64>(6)?,
                    r.get::<_, f64>(7)?,
                    r.get::<_, f64>(8)?,
                    r.get::<_, f64>(9)?,
                    r.get::<_, String>(10)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        let (total_ttc, txn_count): (f64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(total_ttc), 0), COUNT(*)
             FROM transactions
             WHERE DATE(created_at) BETWEEN ?1 AND ?2 AND is_voided = 0",
            params![date_from, date_to],
            |r| Ok((r.get::<_, f64>(0)?, r.get::<_, i64>(1)?)),
        ).unwrap_or((0.0, 0));

        (rows, (total_ttc, txn_count))
    };

    // ── 2. Build workbook ────────────────────────────────────────────────────
    let mut workbook = Workbook::new();

    // -- Header format: bold, light-blue background, centred
    let header_fmt = Format::new()
        .set_bold()
        .set_align(FormatAlign::Center)
        .set_border(FormatBorder::Thin);

    // -- Number format
    let num_fmt = Format::new()
        .set_num_format("#,##0.00");

    // ── Sheet 1: Transactions ─────────────────────────────────────────────────
    let ws = workbook.add_worksheet();
    ws.set_name("Transactions").map_err(|e| e.to_string())?;

    let headers = [
        "Référence", "Date", "Caissier", "Client",
        "Mode paiement", "Total HT (DZD)", "Total TTC (DZD)",
        "Remise (DZD)", "Montant payé (DZD)", "Monnaie (DZD)", "Statut",
    ];

    for (col, h) in headers.iter().enumerate() {
        ws.write_string_with_format(0, col as u16, *h, &header_fmt)
            .map_err(|e| e.to_string())?;
        ws.set_column_width(col as u16, 18.0).map_err(|e| e.to_string())?;
    }

    for (row_idx, row) in rows.iter().enumerate() {
        let r = (row_idx + 1) as u32;
        ws.write_string(r, 0, &row.0).map_err(|e| e.to_string())?;
        ws.write_string(r, 1, &row.1).map_err(|e| e.to_string())?;
        ws.write_string(r, 2, &row.2).map_err(|e| e.to_string())?;
        ws.write_string(r, 3, &row.3).map_err(|e| e.to_string())?;
        ws.write_string(r, 4, &row.4).map_err(|e| e.to_string())?;
        ws.write_number_with_format(r, 5,  row.5, &num_fmt).map_err(|e| e.to_string())?;
        ws.write_number_with_format(r, 6,  row.6, &num_fmt).map_err(|e| e.to_string())?;
        ws.write_number_with_format(r, 7,  row.7, &num_fmt).map_err(|e| e.to_string())?;
        ws.write_number_with_format(r, 8,  row.8, &num_fmt).map_err(|e| e.to_string())?;
        ws.write_number_with_format(r, 9,  row.9, &num_fmt).map_err(|e| e.to_string())?;
        ws.write_string(r, 10, &row.10).map_err(|e| e.to_string())?;
    }

    // Summary row
    let summary_row = (rows.len() + 2) as u32;
    ws.write_string_with_format(summary_row, 0, "TOTAL (non annulés)", &header_fmt)
        .map_err(|e| e.to_string())?;
    ws.write_number_with_format(summary_row, 6, totals.0, &num_fmt)
        .map_err(|e| e.to_string())?;
    ws.write_string(summary_row, 10, &format!("{} transaction(s)", totals.1))
        .map_err(|e| e.to_string())?;

    // ── 3. Save to exports directory ─────────────────────────────────────────
    let app_dir    = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let export_dir = app_dir.join("exports");
    std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;

    let filename = format!(
        "ventes_{}_{}.xlsx",
        date_from.replace('-', ""),
        date_to.replace('-', ""),
    );
    let path = export_dir.join(&filename);
    workbook.save(&path).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}