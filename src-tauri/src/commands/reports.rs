// src-tauri/src/commands/reports.rs
//! Extended reporting commands.
//!
//! All queries return plain JSON-serialisable structs.
//! Heavy aggregations are done in SQLite — never in JavaScript.

use rusqlite::params;
use serde::Serialize;
use tauri::{command, State};

use crate::AppState;

// ─── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DailyReport {
    pub date:              String,
    pub total_sales:       f64,
    pub total_ht:          f64,
    pub total_vat:         f64,
    pub transaction_count: i64,
    pub avg_basket:        f64,
    pub cash_total:        f64,
    pub cib_total:         f64,
    pub dain_total:        f64,
}

#[derive(Debug, Serialize)]
pub struct DateRangeReport {
    pub date_from:         String,
    pub date_to:           String,
    pub total_sales:       f64,
    pub total_ht:          f64,
    pub total_vat:         f64,
    pub transaction_count: i64,
    pub avg_basket:        f64,
    pub daily_breakdown:   Vec<DailyBreakdown>,
}

#[derive(Debug, Serialize)]
pub struct DailyBreakdown {
    pub date:  String,
    pub sales: f64,
    pub txns:  i64,
}

#[derive(Debug, Serialize)]
pub struct ProductSalesRow {
    pub product_id:   i64,
    pub product_name: String,
    pub gtin:         Option<String>,
    pub category:     Option<String>,
    pub qty_sold:     f64,
    pub revenue_ht:   f64,
    pub revenue_ttc:  f64,
    pub txn_count:    i64,
}

#[derive(Debug, Serialize)]
pub struct HourlyHeatmapRow {
    pub hour:  i64,   // 0–23
    pub sales: f64,
    pub txns:  i64,
}

#[derive(Debug, Serialize)]
pub struct PaymentBreakdown {
    pub method:  String,
    pub total:   f64,
    pub count:   i64,
    pub percent: f64,
}

#[derive(Debug, Serialize)]
pub struct FullReport {
    pub summary:           DateRangeReport,
    pub top_products:      Vec<ProductSalesRow>,
    pub hourly_heatmap:    Vec<HourlyHeatmapRow>,
    pub payment_breakdown: Vec<PaymentBreakdown>,
}

// ─── Commands ────────────────────────────────────────────────────────────────

/// Single-day report (used by the POS daily summary).
#[command]
pub async fn cmd_get_daily_report(
    state: State<'_, AppState>,
    date:  String,
) -> Result<DailyReport, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    conn.query_row(
        "SELECT
            ?1                                                            AS date,
            COALESCE(SUM(total_ttc), 0)                                  AS sales,
            COALESCE(SUM(total_ht), 0)                                   AS ht,
            COALESCE(SUM(total_ttc) - SUM(total_ht), 0)                  AS vat,
            COUNT(*)                                                      AS cnt,
            CASE WHEN COUNT(*) > 0 THEN SUM(total_ttc)/COUNT(*) ELSE 0 END AS avg,
            COALESCE(SUM(CASE WHEN payment_method='cash'
                              THEN total_ttc END), 0)                    AS cash,
            COALESCE(SUM(CASE WHEN payment_method IN ('cib','dahabia')
                              THEN total_ttc END), 0)                    AS cib,
            COALESCE(SUM(CASE WHEN payment_method='dain'
                              THEN total_ttc END), 0)                    AS dain
         FROM transactions
         WHERE DATE(created_at) = ?1",
        params![date],
        |r| {
            Ok(DailyReport {
                date:              r.get(0)?,
                total_sales:       r.get(1)?,
                total_ht:          r.get(2)?,
                total_vat:         r.get(3)?,
                transaction_count: r.get(4)?,
                avg_basket:        r.get(5)?,
                cash_total:        r.get(6)?,
                cib_total:         r.get(7)?,
                dain_total:        r.get(8)?,
            })
        },
    ).map_err(|e| e.to_string())
}

/// Full analytical report for a date range.
#[command]
pub async fn cmd_get_full_report(
    state:     State<'_, AppState>,
    date_from: String,
    date_to:   String,
) -> Result<FullReport, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    // ── Summary ───────────────────────────────────────────────────────────
    let summary: DateRangeReport = {
        let row = conn.query_row(
            "SELECT
                COALESCE(SUM(total_ttc), 0),
                COALESCE(SUM(total_ht), 0),
                COALESCE(SUM(total_ttc) - SUM(total_ht), 0),
                COUNT(*),
                CASE WHEN COUNT(*) > 0 THEN SUM(total_ttc)/COUNT(*) ELSE 0 END
             FROM transactions
             WHERE DATE(created_at) BETWEEN ?1 AND ?2",
            params![date_from, date_to],
            |r| Ok((
                r.get::<_,f64>(0)?,
                r.get::<_,f64>(1)?,
                r.get::<_,f64>(2)?,
                r.get::<_,i64>(3)?,
                r.get::<_,f64>(4)?,
            )),
        ).map_err(|e| e.to_string())?;

        // Daily breakdown
        let mut stmt = conn.prepare(
            "SELECT DATE(created_at), COALESCE(SUM(total_ttc),0), COUNT(*)
             FROM transactions
             WHERE DATE(created_at) BETWEEN ?1 AND ?2
             GROUP BY DATE(created_at)
             ORDER BY DATE(created_at)"
        ).map_err(|e| e.to_string())?;
        let daily: Vec<DailyBreakdown> = stmt.query_map(params![date_from, date_to], |r| {
            Ok(DailyBreakdown { date: r.get(0)?, sales: r.get(1)?, txns: r.get(2)? })
        }).map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        DateRangeReport {
            date_from:         date_from.clone(),
            date_to:           date_to.clone(),
            total_sales:       row.0,
            total_ht:          row.1,
            total_vat:         row.2,
            transaction_count: row.3,
            avg_basket:        row.4,
            daily_breakdown:   daily,
        }
    };

    // ── Top products by revenue ───────────────────────────────────────────
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name_fr, p.gtin, c.name_fr AS cat,
                SUM(ti.quantity)                                         AS qty,
                SUM(ti.quantity * ti.unit_price * (1-ti.discount_pct/100.0)) AS ht,
                SUM(ti.quantity * ti.unit_price * (1+ti.vat_rate)
                    * (1-ti.discount_pct/100.0))                        AS ttc,
                COUNT(DISTINCT ti.transaction_id)                       AS txns
         FROM transaction_items ti
         JOIN products p ON p.id = ti.product_id
         JOIN transactions t ON t.id = ti.transaction_id
         LEFT JOIN categories c ON c.id = p.category_id
         WHERE DATE(t.created_at) BETWEEN ?1 AND ?2
         GROUP BY p.id
         ORDER BY ttc DESC
         LIMIT 20"
    ).map_err(|e| e.to_string())?;

    let top_products: Vec<ProductSalesRow> = stmt
        .query_map(params![date_from, date_to], |r| {
            Ok(ProductSalesRow {
                product_id:   r.get(0)?,
                product_name: r.get(1)?,
                gtin:         r.get(2)?,
                category:     r.get(3)?,
                qty_sold:     r.get(4)?,
                revenue_ht:   r.get(5)?,
                revenue_ttc:  r.get(6)?,
                txn_count:    r.get(7)?,
            })
        }).map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // ── Hourly heatmap ────────────────────────────────────────────────────
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', created_at) AS INTEGER) AS hr,
                COALESCE(SUM(total_ttc), 0), COUNT(*)
         FROM transactions
         WHERE DATE(created_at) BETWEEN ?1 AND ?2
         GROUP BY hr
         ORDER BY hr"
    ).map_err(|e| e.to_string())?;
    let hourly_heatmap: Vec<HourlyHeatmapRow> = stmt
        .query_map(params![date_from, date_to], |r| {
            Ok(HourlyHeatmapRow { hour: r.get(0)?, sales: r.get(1)?, txns: r.get(2)? })
        }).map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // ── Payment breakdown ─────────────────────────────────────────────────
    let grand_total = summary.total_sales;
    let mut stmt = conn.prepare(
        "SELECT payment_method,
                COALESCE(SUM(total_ttc),0) AS total,
                COUNT(*)                   AS cnt
         FROM transactions
         WHERE DATE(created_at) BETWEEN ?1 AND ?2
         GROUP BY payment_method
         ORDER BY total DESC"
    ).map_err(|e| e.to_string())?;
    let payment_breakdown: Vec<PaymentBreakdown> = stmt
        .query_map(params![date_from, date_to], |r| {
            let total: f64 = r.get(1)?;
            let pct = if grand_total > 0.0 { (total / grand_total) * 100.0 } else { 0.0 };
            Ok(PaymentBreakdown {
                method:  r.get(0)?,
                total,
                count:   r.get(2)?,
                percent: (pct * 10.0).round() / 10.0,
            })
        }).map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(FullReport {
        summary,
        top_products,
        hourly_heatmap,
        payment_breakdown,
    })
}