// src-tauri/src/commands/inventory.rs
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{command, State};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct InventoryBatch {
    pub id:           i64,
    pub product_id:   i64,
    pub product_name: String,
    pub quantity:     f64,
    pub expiry_date:  Option<String>,
    pub supplier_ref: Option<String>,
    pub cost_price:   Option<f64>,
    pub received_at:  String,
    /// days_until_expiry: negative = already expired, None = no expiry
    pub days_until_expiry: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AddBatchInput {
    pub product_id:  i64,
    pub quantity:    f64,
    pub expiry_date: Option<String>,
    pub supplier_ref: Option<String>,
    pub cost_price:  Option<f64>,
}

#[command]
pub async fn cmd_get_inventory_batches(
    state: State<'_, AppState>,
) -> Result<Vec<InventoryBatch>, String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;

    let mut stmt = conn.prepare("
        SELECT ib.id, ib.product_id, p.name_fr,
               ib.quantity, ib.expiry_date, ib.supplier_ref,
               ib.cost_price, ib.received_at,
               CASE WHEN ib.expiry_date IS NOT NULL THEN
                   CAST(julianday(ib.expiry_date) - julianday('now') AS INTEGER)
               END AS days_until_expiry
        FROM inventory_batches ib
        JOIN products p ON p.id = ib.product_id
        WHERE ib.quantity > 0
        ORDER BY ib.expiry_date ASC NULLS LAST, p.name_fr
    ").map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |r| {
        Ok(InventoryBatch {
            id:                r.get(0)?,
            product_id:        r.get(1)?,
            product_name:      r.get(2)?,
            quantity:          r.get(3)?,
            expiry_date:       r.get(4)?,
            supplier_ref:      r.get(5)?,
            cost_price:        r.get(6)?,
            received_at:       r.get(7)?,
            days_until_expiry: r.get(8)?,
        })
    }).map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[command]
pub async fn cmd_add_inventory_batch(
    state: State<'_, AppState>,
    input: AddBatchInput,
) -> Result<i64, String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;

    conn.execute(
        "INSERT INTO inventory_batches (product_id, quantity, expiry_date, supplier_ref, cost_price)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            input.product_id, input.quantity, input.expiry_date,
            input.supplier_ref, input.cost_price,
        ],
    ).map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

/// Return batches expiring within `warn_days` days (configurable in settings).
#[command]
pub async fn cmd_get_expiry_alerts(
    state: State<'_, AppState>,
    warn_days: Option<i64>,
) -> Result<Vec<InventoryBatch>, String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;
    let threshold = warn_days.unwrap_or(30);

    let mut stmt = conn.prepare("
        SELECT ib.id, ib.product_id, p.name_fr,
               ib.quantity, ib.expiry_date, ib.supplier_ref,
               ib.cost_price, ib.received_at,
               CAST(julianday(ib.expiry_date) - julianday('now') AS INTEGER) AS days
        FROM inventory_batches ib
        JOIN products p ON p.id = ib.product_id
        WHERE ib.quantity > 0
          AND ib.expiry_date IS NOT NULL
          AND julianday(ib.expiry_date) - julianday('now') <= ?1
        ORDER BY ib.expiry_date ASC
    ").map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![threshold], |r| {
        Ok(InventoryBatch {
            id:                r.get(0)?,
            product_id:        r.get(1)?,
            product_name:      r.get(2)?,
            quantity:          r.get(3)?,
            expiry_date:       r.get(4)?,
            supplier_ref:      r.get(5)?,
            cost_price:        r.get(6)?,
            received_at:       r.get(7)?,
            days_until_expiry: r.get(8)?,
        })
    }).map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}