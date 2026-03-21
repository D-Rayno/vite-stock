// src-tauri/src/commands/a4print.rs
//! A4 PDF export commands (Enterprise feature gate: A4_REPORTS).
//!
//! All commands:
//!   1. Query the DB in Rust (no raw data ever sent to frontend)
//!   2. Build the PDF via `utils::pdf`
//!   3. Write to `<AppData>/exports/<name>.pdf`
//!   4. Return the absolute path so the frontend can open it via
//!      `tauri-plugin-opener`

use chrono::Utc;
use rusqlite::params;
use tauri::{command, AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::{
    license::features,
    utils::pdf::{
        self, DainEntryPdf, DainStatementData, ShopInfo, StockReportData, StockReportRow,
    },
    AppState,
};

// ─── helpers ──────────────────────────────────────────────────────────────────

fn require_a4(state: &AppState) -> Result<(), String> {
    let lic = state.license.lock().unwrap();
    if !lic.has_feature(features::A4_REPORTS) {
        return Err("Rapports A4 non activés sur cette licence.".into());
    }
    Ok(())
}

fn exports_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dir  = base.join("exports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn shop_settings(conn: &rusqlite::Connection) -> (String, String, String, String, String) {
    let get = |key: &str| -> String {
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_default()
    };
    (
        get("shop_name_fr"),
        get("shop_address"),
        get("shop_phone"),
        get("shop_nif"),
        get("shop_nis"),
    )
}

// ─── cmd_export_dain_pdf ──────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct ExportPdfResult {
    pub path: String,
}

/// Generate a full A4 Dain ledger statement for a customer and open it.
#[command]
pub async fn cmd_export_dain_pdf(
    app:         AppHandle,
    state:       State<'_, AppState>,
    customer_id: i64,
) -> Result<ExportPdfResult, String> {
    require_a4(&state)?;

    let now      = Utc::now();
    let date_str = now.format("%d/%m/%Y %H:%M").to_string();
    let doc_ref  = format!("DAIN-{}-{}", customer_id, now.format("%Y%m%d%H%M%S"));

    // Collect data under a short lock
    let (customer_name, customer_phone, balance, credit_limit, entries, shop) = {
        let db   = state.db.lock().unwrap();
        let conn = &db.0;

        let (shop_name, addr, phone, nif, nis) = shop_settings(conn);

        let (cust_name, cust_phone, credit) = conn.query_row(
            "SELECT name, phone, COALESCE(credit_limit_dzd, 0)
             FROM customers WHERE id = ?1",
            params![customer_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?)),
        ).map_err(|e| format!("Client introuvable: {e}"))?;

        let balance: f64 = conn.query_row(
            "SELECT COALESCE(SUM(CASE WHEN entry_type='debt' THEN amount ELSE -amount END), 0)
             FROM dain_entries WHERE customer_id = ?1",
            params![customer_id],
            |r| r.get(0),
        ).unwrap_or(0.0);

        let mut stmt = conn.prepare(
            "SELECT entry_type, amount,
                    COALESCE(notes, ''),
                    created_at,
                    COALESCE(balance_after, 0)
             FROM dain_entries
             WHERE customer_id = ?1
             ORDER BY created_at DESC"
        ).map_err(|e| e.to_string())?;

        let entries: Vec<DainEntryPdf> = stmt.query_map(params![customer_id], |r| {
            let entry_type: String = r.get(0)?;
            Ok(DainEntryPdf {
                entry_type:    if entry_type == "debt" { "Débit".into() } else { "Remboursement".into() },
                amount:        r.get(1)?,
                notes:         r.get(2)?,
                date:          r.get::<_, String>(3)?.chars().take(16).collect::<String>().replace('T', " "),
                balance_after: r.get(4)?,
            })
        }).map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        let shop = (shop_name, addr, phone, nif, nis);
        (cust_name, cust_phone, balance, credit, entries, shop)
    };

    let pdf_bytes = pdf::build_dain_statement(&DainStatementData {
        shop:           ShopInfo { name: &shop.0, address: &shop.1, phone: &shop.2, nif: &shop.3, nis: &shop.4 },
        customer_name:  &customer_name,
        customer_phone: &customer_phone,
        balance,
        credit_limit,
        entries:        &entries,
        generated_at:   &date_str,
        doc_ref:        &doc_ref,
    })?;

    let filename = format!("dain_{customer_id}_{}.pdf", now.format("%Y%m%d_%H%M%S"));
    let dir      = exports_dir(&app)?;
    let path     = pdf::write_pdf_to_file(pdf_bytes, &dir, &filename)?;
    let path_str = path.to_string_lossy().to_string();

    // Open with system PDF viewer
    let _ = app.opener().open_path(&path_str, None::<&str>);

    Ok(ExportPdfResult { path: path_str })
}

// ─── cmd_export_stock_pdf ─────────────────────────────────────────────────────

/// Generate an A4 inventory / stock report and open it.
#[command]
pub async fn cmd_export_stock_pdf(
    app:       AppHandle,
    state:     State<'_, AppState>,
    warn_only: Option<bool>,
) -> Result<ExportPdfResult, String> {
    require_a4(&state)?;

    let now        = Utc::now();
    let date_str   = now.format("%d/%m/%Y %H:%M").to_string();
    let warn       = warn_only.unwrap_or(false);

    let (rows, shop) = {
        let db   = state.db.lock().unwrap();
        let conn = &db.0;

        let (shop_name, addr, phone, nif, nis) = shop_settings(conn);

        let warn_days: i64 = conn.query_row(
            "SELECT CAST(value AS INTEGER) FROM settings WHERE key='expiry_warn_days'",
            [],
            |r| r.get(0),
        ).unwrap_or(30);

        let sql = if warn {
            "SELECT p.name_fr, COALESCE(p.gtin,''), COALESCE(c.name_fr,'Divers'),
                    ib.quantity, COALESCE(u.label_fr,'pcs'),
                    COALESCE(ib.expiry_date,''),
                    CASE WHEN ib.expiry_date IS NOT NULL THEN
                        CAST(julianday(ib.expiry_date) - julianday('now') AS INTEGER)
                    END AS days,
                    ib.cost_price
             FROM inventory_batches ib
             JOIN products p ON p.id = ib.product_id
             LEFT JOIN categories c ON c.id = p.category_id
             LEFT JOIN units u ON u.id = p.unit_id
             WHERE ib.quantity > 0 AND ib.is_active = 1
               AND ib.expiry_date IS NOT NULL
               AND julianday(ib.expiry_date) - julianday('now') <= ?1
             ORDER BY ib.expiry_date ASC"
        } else {
            "SELECT p.name_fr, COALESCE(p.gtin,''), COALESCE(c.name_fr,'Divers'),
                    ib.quantity, COALESCE(u.label_fr,'pcs'),
                    COALESCE(ib.expiry_date,''),
                    CASE WHEN ib.expiry_date IS NOT NULL THEN
                        CAST(julianday(ib.expiry_date) - julianday('now') AS INTEGER)
                    END AS days,
                    ib.cost_price
             FROM inventory_batches ib
             JOIN products p ON p.id = ib.product_id
             LEFT JOIN categories c ON c.id = p.category_id
             LEFT JOIN units u ON u.id = p.unit_id
             WHERE ib.quantity > 0 AND ib.is_active = 1
             ORDER BY p.name_fr ASC, ib.expiry_date ASC NULLS LAST"
        };

        let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows: Vec<StockReportRow> = stmt.query_map(params![warn_days], |r| {
            Ok(StockReportRow {
                product_name: r.get(0)?,
                gtin:         r.get(1)?,
                category:     r.get(2)?,
                quantity:     r.get(3)?,
                unit:         r.get(4)?,
                expiry_date:  r.get(5)?,
                days_left:    r.get(6)?,
                cost_price:   r.get(7)?,
            })
        }).map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        let shop = (shop_name, addr, phone, nif, nis);
        (rows, shop)
    };

    let pdf_bytes = pdf::build_stock_report(&StockReportData {
        shop:         ShopInfo { name: &shop.0, address: &shop.1, phone: &shop.2, nif: &shop.3, nis: &shop.4 },
        rows:         &rows,
        generated_at: &date_str,
        warn_only:    warn,
    })?;

    let prefix   = if warn { "alertes" } else { "inventaire" };
    let filename = format!("stock_{prefix}_{}.pdf", now.format("%Y%m%d_%H%M%S"));
    let dir      = exports_dir(&app)?;
    let path     = pdf::write_pdf_to_file(pdf_bytes, &dir, &filename)?;
    let path_str = path.to_string_lossy().to_string();

    let _ = app.opener().open_path(&path_str, None::<&str>);

    Ok(ExportPdfResult { path: path_str })
}

// ─── cmd_export_sales_pdf ─────────────────────────────────────────────────────

/// Generate a dated sales summary PDF and open it.
/// Uses the same data as the Excel export but renders as A4 PDF.
#[command]
pub async fn cmd_export_sales_pdf(
    app:       AppHandle,
    state:     State<'_, AppState>,
    date_from: String,
    date_to:   String,
) -> Result<ExportPdfResult, String> {
    require_a4(&state)?;

    let now      = Utc::now();
    let date_str = now.format("%d/%m/%Y %H:%M").to_string();

    let (rows, totals, shop) = {
        let db   = state.db.lock().unwrap();
        let conn = &db.0;
        let (shop_name, addr, phone, nif, nis) = shop_settings(conn);

        let mut stmt = conn.prepare(
            "SELECT t.ref_number, DATE(t.created_at),
                    COALESCE(c.name,'—'), t.payment_method,
                    t.total_ttc, t.cashier_name
             FROM transactions t
             LEFT JOIN customers c ON c.id = t.customer_id
             WHERE DATE(t.created_at) BETWEEN ?1 AND ?2
               AND t.is_voided = 0
             ORDER BY t.created_at"
        ).map_err(|e| e.to_string())?;

        let rows: Vec<Vec<(String, Option<pdf::Color>)>> = stmt.query_map(
            params![date_from, date_to],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, f64>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        ).map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .map(|(ref_num, date, customer, method, total, cashier)| {
            vec![
                (ref_num, None),
                (date, None),
                (customer, None),
                (method, None),
                (format!("{:.2}", total), None),
                (cashier, None),
            ]
        })
        .collect();

        let (total_ttc, txn_count): (f64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(total_ttc),0), COUNT(*)
             FROM transactions
             WHERE DATE(created_at) BETWEEN ?1 AND ?2 AND is_voided=0",
            params![date_from, date_to],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap_or((0.0, 0));

        let shop = (shop_name, addr, phone, nif, nis);
        (rows, (total_ttc, txn_count), shop)
    };

    // Build PDF manually (sales report is a simple table)
    let doc_title = format!("RAPPORT VENTES du {} au {}", date_from, date_to);
    let mut c = pdf::PdfCanvas::new(&doc_title);
    let doc_ref = format!("RPT-{}", now.format("%Y%m%d%H%M%S"));

    c.header(
        &shop.0, &shop.1, &shop.2, &shop.3, &shop.4,
        "RAPPORT DES VENTES",
        &doc_ref,
        &date_str,
    );

    c.section_title(&format!("Période : {} → {}", date_from, date_to));
    c.kv_row("Nb transactions :", &totals.1.to_string(), 9.0);
    c.kv_row("Total TTC :",       &format!("{:.2} DZD", totals.0), 9.0);
    c.kv_row("Panier moyen :",    &format!("{:.2} DZD", if totals.1 > 0 { totals.0 / totals.1 as f64 } else { 0.0 }), 9.0);
    c.gap(3.0);

    c.section_title("Détail des transactions");
    let cols: &[(&str, f64, u8)] = &[
        ("Référence",     36.0, 0),
        ("Date",          22.0, 0),
        ("Client",        30.0, 0),
        ("Mode",          22.0, 0),
        ("Total TTC",     28.0, 1),
        ("Caissier",      32.0, 0),
    ];
    c.table(cols, &rows);

    c.add_footers(&date_str);

    let pdf_bytes = c.save()?;
    let filename  = format!("ventes_{}_{}.pdf", date_from.replace('-', ""), date_to.replace('-', ""));
    let dir       = exports_dir(&app)?;
    let path      = pdf::write_pdf_to_file(pdf_bytes, &dir, &filename)?;
    let path_str  = path.to_string_lossy().to_string();

    let _ = app.opener().open_path(&path_str, None::<&str>);

    Ok(ExportPdfResult { path: path_str })
}