// src-tauri/src/commands/products.rs
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{command, State};

use crate::AppState;

// ─── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProductRow {
    pub id:               i64,
    pub gtin:             Option<String>,
    pub name_fr:          String,
    pub name_ar:          String,
    pub category_id:      Option<i64>,
    pub category_name_fr: Option<String>,
    pub unit_id:          Option<i64>,
    pub unit_label_fr:    Option<String>,
    pub sell_price:       f64,
    pub buy_price:        f64,
    pub vat_rate:         f64,
    pub min_stock_alert:  i64,
    pub is_active:        bool,
    /// Aggregated available quantity across all batches.
    pub total_stock:      f64,
    pub created_at:       String,
}

/// Returned by `lookup_product` — includes the FEFO-selected batch.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProductLookupResult {
    // ── Product fields ────────────────────────────────────────────────────
    pub id:            i64,
    pub gtin:          Option<String>,
    pub name_fr:       String,
    pub name_ar:       String,
    pub sell_price:    f64,
    pub vat_rate:      f64,
    pub unit_label_fr: Option<String>,
    pub total_stock:   f64,
    // ── FEFO batch fields (null if no batch / no stock) ───────────────────
    pub batch_id:      Option<i64>,
    pub batch_qty:     Option<f64>,
    pub expiry_date:   Option<String>,
    /// days_until_expiry: negative = already expired, None = no expiry on batch
    pub days_until_expiry: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductInput {
    pub gtin:            Option<String>,
    pub name_fr:         String,
    pub name_ar:         String,
    pub category_id:     Option<i64>,
    pub unit_id:         Option<i64>,
    pub sell_price:      f64,
    pub buy_price:       f64,
    pub vat_rate:        Option<f64>,
    pub min_stock_alert: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProductInput {
    pub id:              i64,
    pub gtin:            Option<String>,
    pub name_fr:         String,
    pub name_ar:         String,
    pub category_id:     Option<i64>,
    pub unit_id:         Option<i64>,
    pub sell_price:      f64,
    pub buy_price:       f64,
    pub vat_rate:        f64,
    pub min_stock_alert: i64,
    pub is_active:       bool,
}

// ─── Commands ────────────────────────────────────────────────────────────────

/// Look up a product by GTIN (barcode) scan.
/// Applies FEFO: joins the batch with the earliest non-null expiry date first,
/// falling back to batches with no expiry date.
/// Returns `None` if no active product matches the GTIN.
#[command]
pub async fn cmd_lookup_product(
    state: State<'_, AppState>,
    gtin:  String,
) -> Result<Option<ProductLookupResult>, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    // Step 1: find the product.
    let product_opt = conn.query_row(
        "SELECT p.id, p.gtin, p.name_fr, p.name_ar, p.sell_price, p.vat_rate,
                u.label_fr AS unit_label_fr,
                COALESCE(SUM(ib2.quantity), 0) AS total_stock
         FROM products p
         LEFT JOIN units u            ON u.id = p.unit_id
         LEFT JOIN inventory_batches ib2 ON ib2.product_id = p.id
         WHERE p.gtin = ?1 AND p.is_active = 1
         GROUP BY p.id
         LIMIT 1",
        params![gtin],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, f64>(7)?,
            ))
        },
    ).optional().map_err(|e| e.to_string())?;

    let Some((pid, p_gtin, name_fr, name_ar, sell_price, vat_rate, unit_label_fr, total_stock))
        = product_opt
    else {
        return Ok(None);
    };

    // Step 2: FEFO — pick the batch expiring soonest (with stock > 0).
    // Batches with a real expiry_date come before NULL-expiry batches.
    let batch_opt = conn.query_row(
        "SELECT ib.id, ib.quantity, ib.expiry_date,
                CASE WHEN ib.expiry_date IS NOT NULL THEN
                    CAST(julianday(ib.expiry_date) - julianday('now') AS INTEGER)
                END AS days
         FROM inventory_batches ib
         WHERE ib.product_id = ?1 AND ib.quantity > 0
         ORDER BY
             CASE WHEN ib.expiry_date IS NULL THEN 1 ELSE 0 END ASC,  -- dated first
             ib.expiry_date ASC                                         -- then soonest
         LIMIT 1",
        params![pid],
        |r| Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, f64>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<i64>>(3)?,
        )),
    ).optional().map_err(|e| e.to_string())?;

    let (batch_id, batch_qty, expiry_date, days_until_expiry) = batch_opt
        .map(|(id, qty, exp, days)| (Some(id), Some(qty), exp, days))
        .unwrap_or((None, None, None, None));

    Ok(Some(ProductLookupResult {
        id: pid,
        gtin: p_gtin,
        name_fr,
        name_ar,
        sell_price,
        vat_rate,
        unit_label_fr,
        total_stock,
        batch_id,
        batch_qty,
        expiry_date,
        days_until_expiry,
    }))
}

#[command]
pub async fn cmd_get_products(state: State<'_, AppState>) -> Result<Vec<ProductRow>, String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;

    let mut stmt = conn.prepare("
        SELECT
            p.id, p.gtin, p.name_fr, p.name_ar,
            p.category_id, c.name_fr AS category_name_fr,
            p.unit_id,     u.label_fr AS unit_label_fr,
            p.sell_price, p.buy_price, p.vat_rate,
            p.min_stock_alert, p.is_active, p.created_at,
            COALESCE(SUM(ib.quantity), 0) AS total_stock
        FROM products p
        LEFT JOIN categories c ON c.id = p.category_id
        LEFT JOIN units u      ON u.id = p.unit_id
        LEFT JOIN inventory_batches ib ON ib.product_id = p.id
        GROUP BY p.id
        ORDER BY p.name_fr
    ").map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |r| {
        Ok(ProductRow {
            id:               r.get(0)?,
            gtin:             r.get(1)?,
            name_fr:          r.get(2)?,
            name_ar:          r.get(3)?,
            category_id:      r.get(4)?,
            category_name_fr: r.get(5)?,
            unit_id:          r.get(6)?,
            unit_label_fr:    r.get(7)?,
            sell_price:       r.get(8)?,
            buy_price:        r.get(9)?,
            vat_rate:         r.get(10)?,
            min_stock_alert:  r.get(11)?,
            is_active:        r.get::<_, i64>(12)? == 1,
            created_at:       r.get(13)?,
            total_stock:      r.get(14)?,
        })
    }).map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[command]
pub async fn cmd_search_products(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<ProductRow>, String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;
    let pattern = format!("%{}%", query.to_lowercase());

    let mut stmt = conn.prepare("
        SELECT
            p.id, p.gtin, p.name_fr, p.name_ar,
            p.category_id, c.name_fr,
            p.unit_id,     u.label_fr,
            p.sell_price, p.buy_price, p.vat_rate,
            p.min_stock_alert, p.is_active, p.created_at,
            COALESCE(SUM(ib.quantity), 0) AS total_stock
        FROM products p
        LEFT JOIN categories c ON c.id = p.category_id
        LEFT JOIN units u      ON u.id = p.unit_id
        LEFT JOIN inventory_batches ib ON ib.product_id = p.id
        WHERE p.is_active = 1
          AND (LOWER(p.name_fr) LIKE ?1
            OR LOWER(p.name_ar) LIKE ?1
            OR p.gtin           LIKE ?1)
        GROUP BY p.id
        ORDER BY p.name_fr
        LIMIT 50
    ").map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![pattern], |r| {
        Ok(ProductRow {
            id:               r.get(0)?,
            gtin:             r.get(1)?,
            name_fr:          r.get(2)?,
            name_ar:          r.get(3)?,
            category_id:      r.get(4)?,
            category_name_fr: r.get(5)?,
            unit_id:          r.get(6)?,
            unit_label_fr:    r.get(7)?,
            sell_price:       r.get(8)?,
            buy_price:        r.get(9)?,
            vat_rate:         r.get(10)?,
            min_stock_alert:  r.get(11)?,
            is_active:        r.get::<_, i64>(12)? == 1,
            created_at:       r.get(13)?,
            total_stock:      r.get(14)?,
        })
    }).map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[command]
pub async fn cmd_create_product(
    state: State<'_, AppState>,
    input: CreateProductInput,
) -> Result<i64, String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;

    conn.execute(
        "INSERT INTO products (gtin, name_fr, name_ar, category_id, unit_id,
                               sell_price, buy_price, vat_rate, min_stock_alert)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            input.gtin,
            input.name_fr,
            input.name_ar,
            input.category_id,
            input.unit_id,
            input.sell_price,
            input.buy_price,
            input.vat_rate.unwrap_or(0.19),
            input.min_stock_alert.unwrap_or(5),
        ],
    ).map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

#[command]
pub async fn cmd_update_product(
    state: State<'_, AppState>,
    input: UpdateProductInput,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;

    conn.execute(
        "UPDATE products
         SET gtin=?1, name_fr=?2, name_ar=?3, category_id=?4, unit_id=?5,
             sell_price=?6, buy_price=?7, vat_rate=?8, min_stock_alert=?9,
             is_active=?10
         WHERE id=?11",
        params![
            input.gtin,
            input.name_fr,
            input.name_ar,
            input.category_id,
            input.unit_id,
            input.sell_price,
            input.buy_price,
            input.vat_rate,
            input.min_stock_alert,
            input.is_active as i64,
            input.id,
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[command]
pub async fn cmd_delete_product(
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;
    // Soft-delete: set is_active = 0 to preserve historical transaction records.
    conn.execute(
        "UPDATE products SET is_active = 0 WHERE id = ?1",
        params![id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}