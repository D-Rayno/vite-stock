// src-tauri/src/background.rs
//! Background polling thread for proactive alert notifications.
//!
//! Spawned once at app startup via `tauri::async_runtime::spawn`.
//! The mutex is held only for the duration of each SQL query, then released
//! before any notification I/O so POS operations are never blocked.

use std::time::Duration;
use tauri::{AppHandle, Manager};   // Manager is required for app.state()

use crate::AppState;

// ─── Entry point ──────────────────────────────────────────────────────────────

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Grace period — let the app fully initialise before first check.
        tokio::time::sleep(Duration::from_secs(15)).await;

        loop {
            run_check(&app).await;
            let interval_h = read_interval_hours(&app);
            tokio::time::sleep(Duration::from_secs(interval_h * 3600)).await;
        }
    });
}

// ─── Main check ───────────────────────────────────────────────────────────────

async fn run_check(app: &AppHandle) {
    let (expiry_stats, low_stock_count, warn_days) = collect_alert_data(app);
    let (expired, critical, warning) = expiry_stats;

    if expired.total > 0 {
        let body = format_alert_body(&expired.sample, expired.total, "produit(s) expire(s)");
        let _ = app
            .notification()
            .builder()
            .title("Produits expires — SuperPOS")
            .body(&body)
            .show();
    }

    if critical.total > 0 {
        let body = format!(
            "{} produit(s) expirent dans 7 jours ou moins : {}",
            critical.total,
            critical.sample.join(", "),
        );
        let _ = app
            .notification()
            .builder()
            .title("Expiration imminente — SuperPOS")
            .body(&body)
            .show();
    }

    if warning.total > 0 {
        let body = format!(
            "{} produit(s) expirent dans {} jours ou moins.",
            warning.total, warn_days
        );
        let _ = app
            .notification()
            .builder()
            .title("Expiration proche — SuperPOS")
            .body(&body)
            .show();
    }

    if low_stock_count > 0 {
        let body = format!("{} produit(s) sous le seuil d'alerte de stock.", low_stock_count);
        let _ = app
            .notification()
            .builder()
            .title("Stock bas — SuperPOS")
            .body(&body)
            .show();
    }
}

// ─── Data collection ──────────────────────────────────────────────────────────

struct AlertGroup {
    total:  usize,
    sample: Vec<String>,
}

fn collect_alert_data(app: &AppHandle) -> ((AlertGroup, AlertGroup, AlertGroup), usize, i64) {
    let state = app.state::<AppState>();
    let db    = state.db.lock().unwrap();
    let conn  = &db.0;

    // Read configured warning window; use type annotation to guide inference.
    let warn_days: i64 = conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM settings WHERE key='expiry_warn_days'",
        [],
        |r: &rusqlite::Row<'_>| r.get::<_, i64>(0),
    ).unwrap_or(30);

    // Expiry alerts
    let mut expired_names:  Vec<String> = vec![];
    let mut critical_names: Vec<String> = vec![];
    let mut warning_names:  Vec<String> = vec![];

    if let Ok(mut stmt) = conn.prepare(
        "SELECT p.name_fr,
                CAST(julianday(ib.expiry_date) - julianday('now') AS INTEGER) AS days
         FROM inventory_batches ib
         JOIN products p ON p.id = ib.product_id
         WHERE ib.quantity > 0
           AND ib.expiry_date IS NOT NULL
           AND julianday(ib.expiry_date) - julianday('now') <= ?1
         ORDER BY ib.expiry_date ASC",
    ) {
        // Explicit closure type annotation resolves the inference failure.
        if let Ok(rows) = stmt.query_map(
            [warn_days],
            |r: &rusqlite::Row<'_>| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        ) {
            for row in rows {
                if let Ok((name, days)) = row {
                    if days < 0 {
                        expired_names.push(name);
                    } else if days <= 7 {
                        critical_names.push(name);
                    } else {
                        warning_names.push(name);
                    }
                }
            }
        }
    }

    // Low stock count
    let low_stock_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (
            SELECT p.id
            FROM products p
            LEFT JOIN inventory_batches ib ON ib.product_id = p.id
            WHERE p.is_active = 1
            GROUP BY p.id
            HAVING COALESCE(SUM(ib.quantity), 0) <= p.min_stock_alert
         )",
        [],
        |r: &rusqlite::Row<'_>| r.get::<_, i64>(0),
    ).unwrap_or(0);

    // Release the DB lock before notification I/O.
    drop(db);

    let make_group = |names: Vec<String>| {
        let total  = names.len();
        let sample = names.into_iter().take(3).collect();
        AlertGroup { total, sample }
    };

    (
        (make_group(expired_names), make_group(critical_names), make_group(warning_names)),
        low_stock_count as usize,
        warn_days,
    )
}

fn format_alert_body(names: &[String], total: usize, suffix: &str) -> String {
    let mut body = format!("{} {}: {}", total, suffix, names.join(", "));
    if total > 3 {
        body.push_str(&format!(" (+{})", total - 3));
    }
    body
}

fn read_interval_hours(app: &AppHandle) -> u64 {
    let state = app.state::<AppState>();
    let db    = state.db.lock().unwrap();
    let conn  = &db.0;
    let h: i64 = conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM settings WHERE key='backup_interval_h'",
        [],
        |r: &rusqlite::Row<'_>| r.get::<_, i64>(0),
    ).unwrap_or(6);
    drop(db);
    h.max(1) as u64
}