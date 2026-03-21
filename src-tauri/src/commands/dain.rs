// src-tauri/src/commands/dain.rs
//! Dain (customer credit) commands — gated by Enterprise feature flag.
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{command, State};
use crate::{license::features, AppState};

#[derive(Debug, Serialize)]
pub struct CustomerDainSummary {
    pub customer_id:   i64,
    pub name:          String,
    pub phone:         String,
    pub balance:       f64,    // positive = owes money
}

#[derive(Debug, Serialize)]
pub struct DainEntry {
    pub id:             i64,
    pub entry_type:     String,
    pub amount:         f64,
    pub notes:          Option<String>,
    pub created_at:     String,
}

fn require_dain(state: &AppState) -> Result<(), String> {
    let lic = state.license.lock().unwrap();
    if !lic.has_feature(features::DAIN_LEDGER) {
        return Err("Fonctionnalité Dain non activée sur cette licence.".into());
    }
    Ok(())
}

#[command]
pub async fn cmd_get_customer(
    state: State<'_, AppState>,
    phone: String,
) -> Result<CustomerDainSummary, String> {
    require_dain(&state)?;
    let db = state.db.lock().unwrap();
    let conn = &db.0;

    conn.query_row("
        SELECT c.id, c.name, c.phone,
               COALESCE(SUM(CASE WHEN d.entry_type='debt'      THEN d.amount ELSE 0 END), 0)
             - COALESCE(SUM(CASE WHEN d.entry_type='repayment' THEN d.amount ELSE 0 END), 0)
               AS balance
        FROM customers c
        LEFT JOIN dain_entries d ON d.customer_id = c.id
        WHERE c.phone = ?1
        GROUP BY c.id
    ", params![phone], |r| {
        Ok(CustomerDainSummary {
            customer_id: r.get(0)?,
            name:        r.get(1)?,
            phone:       r.get(2)?,
            balance:     r.get(3)?,
        })
    }).map_err(|e| e.to_string())
}

#[command]
pub async fn cmd_add_dain_entry(
    state: State<'_, AppState>,
    customer_id: i64,
    transaction_id: Option<i64>,
    amount: f64,
    notes: Option<String>,
) -> Result<i64, String> {
    require_dain(&state)?;
    let db = state.db.lock().unwrap();
    let conn = &db.0;
    conn.execute(
        "INSERT INTO dain_entries (customer_id, transaction_id, entry_type, amount, notes)
         VALUES (?1, ?2, 'debt', ?3, ?4)",
        params![customer_id, transaction_id, amount, notes],
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[command]
pub async fn cmd_repay_dain(
    state: State<'_, AppState>,
    customer_id: i64,
    amount: f64,
    notes: Option<String>,
) -> Result<i64, String> {
    require_dain(&state)?;
    let db = state.db.lock().unwrap();
    let conn = &db.0;
    conn.execute(
        "INSERT INTO dain_entries (customer_id, entry_type, amount, notes)
         VALUES (?1, 'repayment', ?2, ?3)",
        params![customer_id, amount, notes],
    ).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[command]
pub async fn cmd_get_dain_history(
    state: State<'_, AppState>,
    customer_id: i64,
) -> Result<Vec<DainEntry>, String> {
    require_dain(&state)?;
    let db = state.db.lock().unwrap();
    let conn = &db.0;

    let mut stmt = conn.prepare("
        SELECT id, entry_type, amount, notes, created_at
        FROM dain_entries WHERE customer_id = ?1 ORDER BY created_at DESC
    ").map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![customer_id], |r| {
        Ok(DainEntry {
            id:         r.get(0)?,
            entry_type: r.get(1)?,
            amount:     r.get(2)?,
            notes:      r.get(3)?,
            created_at: r.get(4)?,
        })
    }).map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}