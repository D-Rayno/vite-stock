// src-tauri/src/commands/alerts.rs
//! Startup alert system.
//!
//! On every app launch, React calls `cmd_run_startup_checks` which:
//!   1. Queries expired / near-expiry inventory batches.
//!   2. Queries products below their minimum stock threshold.
//!   3. Fires native Windows Toast notifications via tauri-plugin-notification.
//!   4. Returns a structured summary so the React dashboard can show banners.

use rusqlite::params;
use serde::Serialize;
use tauri::{command, AppHandle, State};
use tauri_plugin_notification::NotificationExt;

use crate::AppState;

// ─── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct ExpiryAlert {
    pub batch_id:          i64,
    pub product_id:        i64,
    pub product_name:      String,
    pub quantity:          f64,
    pub expiry_date:       String,
    pub days_until_expiry: i64,
    pub status:            ExpiryAlertStatus,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExpiryAlertStatus {
    Expired,
    Critical,   // 0–7 days
    Warning,    // 8–warn_days days
}

#[derive(Debug, Serialize, Clone)]
pub struct LowStockAlert {
    pub product_id:     i64,
    pub product_name:   String,
    pub total_stock:    f64,
    pub min_stock:      i64,
    pub unit_label:     Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StartupCheckResult {
    pub expiry_alerts:    Vec<ExpiryAlert>,
    pub low_stock_alerts: Vec<LowStockAlert>,
    pub expired_count:    usize,
    pub critical_count:   usize,
    pub low_stock_count:  usize,
}

// ─── Main command ─────────────────────────────────────────────────────────────

/// Run all startup checks and fire native notifications.
/// Always succeeds — notification errors are silently swallowed so they
/// never block the app from starting.
#[command]
pub async fn cmd_run_startup_checks(
    app:       AppHandle,
    state:     State<'_, AppState>,
    warn_days: Option<i64>,
) -> Result<StartupCheckResult, String> {
    let threshold = warn_days.unwrap_or(30);

    let (expiry_alerts, low_stock_alerts) = {
        let db   = state.db.lock().unwrap();
        let conn = &db.0;
        (
            query_expiry_alerts(conn, threshold).map_err(|e| e.to_string())?,
            query_low_stock(conn).map_err(|e| e.to_string())?,
        )
    };

    let expired_count  = expiry_alerts.iter().filter(|a| a.status == ExpiryAlertStatus::Expired).count();
    let critical_count = expiry_alerts.iter().filter(|a| a.status == ExpiryAlertStatus::Critical).count();
    let low_stock_count = low_stock_alerts.len();

    // ── Fire native Toast notifications ─────────────────────────────────

    // Notification 1: Expired products (high urgency)
    if expired_count > 0 {
        let names: Vec<&str> = expiry_alerts
            .iter()
            .filter(|a| a.status == ExpiryAlertStatus::Expired)
            .take(3)
            .map(|a| a.product_name.as_str())
            .collect();
        let body = format!(
            "{} produit(s) expiré(s) : {}{}",
            expired_count,
            names.join(", "),
            if expired_count > 3 { format!(" +{}", expired_count - 3) } else { String::new() }
        );
        let _ = app
            .notification()
            .builder()
            .title("⛔ Produits expirés — SuperPOS")
            .body(&body)
            .show();
    }

    // Notification 2: Near-expiry (medium urgency)
    if critical_count > 0 {
        let body = format!(
            "{} produit(s) expirent dans moins de 7 jours.",
            critical_count
        );
        let _ = app
            .notification()
            .builder()
            .title("⚠ Expiration imminente — SuperPOS")
            .body(&body)
            .show();
    }

    // Notification 3: Low stock (low urgency — batch, only if many)
    if low_stock_count > 0 {
        let body = format!(
            "{} produit(s) sous le seuil d'alerte stock.",
            low_stock_count
        );
        let _ = app
            .notification()
            .builder()
            .title("📦 Stock bas — SuperPOS")
            .body(&body)
            .show();
    }

    Ok(StartupCheckResult {
        expiry_alerts,
        low_stock_alerts,
        expired_count,
        critical_count,
        low_stock_count,
    })
}

// ─── Database queries ─────────────────────────────────────────────────────────

fn query_expiry_alerts(
    conn:      &rusqlite::Connection,
    warn_days: i64,
) -> rusqlite::Result<Vec<ExpiryAlert>> {
    let mut stmt = conn.prepare(
        "SELECT ib.id, ib.product_id, p.name_fr,
                ib.quantity, ib.expiry_date,
                CAST(julianday(ib.expiry_date) - julianday('now') AS INTEGER) AS days
         FROM inventory_batches ib
         JOIN products p ON p.id = ib.product_id
         WHERE ib.quantity > 0
           AND ib.expiry_date IS NOT NULL
           AND julianday(ib.expiry_date) - julianday('now') <= ?1
         ORDER BY ib.expiry_date ASC",
    )?;

    let rows = stmt.query_map(params![warn_days], |r| {
        let days: i64 = r.get(5)?;
        let status = if days < 0 {
            ExpiryAlertStatus::Expired
        } else if days <= 7 {
            ExpiryAlertStatus::Critical
        } else {
            ExpiryAlertStatus::Warning
        };
        Ok(ExpiryAlert {
            batch_id:          r.get(0)?,
            product_id:        r.get(1)?,
            product_name:      r.get(2)?,
            quantity:          r.get(3)?,
            expiry_date:       r.get(4)?,
            days_until_expiry: days,
            status,
        })
    })?;

    rows.collect()
}

fn query_low_stock(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<LowStockAlert>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name_fr,
                COALESCE(SUM(ib.quantity), 0) AS total_stock,
                p.min_stock_alert,
                u.label_fr
         FROM products p
         LEFT JOIN inventory_batches ib ON ib.product_id = p.id
         LEFT JOIN units u ON u.id = p.unit_id
         WHERE p.is_active = 1
         GROUP BY p.id
         HAVING total_stock <= p.min_stock_alert
         ORDER BY total_stock ASC",
    )?;

    let rows = stmt.query_map([], |r| {
        Ok(LowStockAlert {
            product_id:   r.get(0)?,
            product_name: r.get(1)?,
            total_stock:  r.get(2)?,
            min_stock:    r.get(3)?,
            unit_label:   r.get(4)?,
        })
    })?;

    rows.collect()
}