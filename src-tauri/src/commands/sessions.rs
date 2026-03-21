// src-tauri/src/commands/sessions.rs
//! Cashier session management (till open/close, float reconciliation).

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{command, State};

use crate::AppState;

#[derive(Debug, Serialize, Clone)]
pub struct CashierSession {
    pub id:               i64,
    pub cashier_name:     String,
    pub opening_float:    f64,
    pub closing_declared: Option<f64>,
    pub total_sales_ttc:  f64,
    pub total_cash_sales: f64,
    pub total_cib_sales:  f64,
    pub total_dain_sales: f64,
    pub expected_cash:    f64,
    pub variance:         Option<f64>,
    pub notes:            Option<String>,
    pub opened_at:        String,
    pub closed_at:        Option<String>,
    pub status:           String,
}

/// Open a new cashier session (shift start).
#[command]
pub async fn cmd_open_session(
    state:         State<'_, AppState>,
    cashier_name:  String,
    opening_float: f64,
) -> Result<i64, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    // Only one session per cashier can be open at a time
    let open_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cashier_sessions WHERE cashier_name = ?1 AND status = 'open'",
        params![cashier_name],
        |r| r.get(0),
    ).unwrap_or(0);

    if open_count > 0 {
        return Err(format!(
            "Une session est déjà ouverte pour {cashier_name}. Fermez-la d'abord."
        ));
    }

    conn.execute(
        "INSERT INTO cashier_sessions (cashier_name, opening_float) VALUES (?1, ?2)",
        params![cashier_name, opening_float],
    ).map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

/// Close the active session for a cashier.
#[command]
pub async fn cmd_close_session(
    state:            State<'_, AppState>,
    session_id:       i64,
    closing_declared: f64,
    notes:            Option<String>,
) -> Result<CashierSession, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    conn.execute(
        "UPDATE cashier_sessions
         SET status = 'closed',
             closing_declared = ?1,
             notes = COALESCE(?2, notes),
             closed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?3 AND status = 'open'",
        params![closing_declared, notes, session_id],
    ).map_err(|e| e.to_string())?;

    get_session(conn, session_id)
}

/// Get the currently open session (if any).
#[command]
pub async fn cmd_get_active_session(
    state:        State<'_, AppState>,
    cashier_name: String,
) -> Result<Option<CashierSession>, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    let row = conn.query_row(
        "SELECT id, cashier_name, opening_float, closing_declared,
                total_sales_ttc, total_cash_sales, total_cib_sales, total_dain_sales,
                expected_cash, variance, notes, opened_at, closed_at, status
         FROM cashier_sessions
         WHERE cashier_name = ?1 AND status = 'open'
         ORDER BY opened_at DESC LIMIT 1",
        params![cashier_name],
        map_session_row,
    ).optional().map_err(|e| e.to_string())?;

    Ok(row)
}

/// List recent sessions (for manager review).
#[command]
pub async fn cmd_list_sessions(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<CashierSession>, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;
    let lim  = limit.unwrap_or(50);

    let mut stmt = conn.prepare("
        SELECT id, cashier_name, opening_float, closing_declared,
               total_sales_ttc, total_cash_sales, total_cib_sales, total_dain_sales,
               expected_cash, variance, notes, opened_at, closed_at, status
        FROM cashier_sessions
        ORDER BY opened_at DESC
        LIMIT ?1
    ").map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![lim], map_session_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(rows)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn get_session(conn: &rusqlite::Connection, id: i64) -> Result<CashierSession, String> {
    conn.query_row(
        "SELECT id, cashier_name, opening_float, closing_declared,
                total_sales_ttc, total_cash_sales, total_cib_sales, total_dain_sales,
                expected_cash, variance, notes, opened_at, closed_at, status
         FROM cashier_sessions WHERE id = ?1",
        params![id],
        map_session_row,
    ).map_err(|e| e.to_string())
}

fn map_session_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<CashierSession> {
    Ok(CashierSession {
        id:               r.get(0)?,
        cashier_name:     r.get(1)?,
        opening_float:    r.get(2)?,
        closing_declared: r.get(3)?,
        total_sales_ttc:  r.get(4)?,
        total_cash_sales: r.get(5)?,
        total_cib_sales:  r.get(6)?,
        total_dain_sales: r.get(7)?,
        expected_cash:    r.get::<_, Option<f64>>(8)?.unwrap_or(0.0),
        variance:         r.get(9)?,
        notes:            r.get(10)?,
        opened_at:        r.get(11)?,
        closed_at:        r.get(12)?,
        status:           r.get(13)?,
    })
}