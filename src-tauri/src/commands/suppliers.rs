// src-tauri/src/commands/suppliers.rs
//! Supplier management and purchase order commands.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{command, State};

use crate::AppState;

// ─── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Supplier {
    pub id:                 i64,
    pub code:               Option<String>,
    pub name:               String,
    pub name_ar:            String,
    pub contact_name:       Option<String>,
    pub phone:              Option<String>,
    pub email:              Option<String>,
    pub address:            Option<String>,
    pub wilaya:             Option<String>,
    pub nif:                Option<String>,
    pub nis:                Option<String>,
    pub rc:                 Option<String>,
    pub payment_terms_days: i64,
    pub total_debt_dzd:     f64,
    pub is_active:          bool,
    pub created_at:         String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSupplierInput {
    pub code:               Option<String>,
    pub name:               String,
    pub name_ar:            Option<String>,
    pub contact_name:       Option<String>,
    pub phone:              Option<String>,
    pub email:              Option<String>,
    pub address:            Option<String>,
    pub wilaya:             Option<String>,
    pub nif:                Option<String>,
    pub nis:                Option<String>,
    pub rc:                 Option<String>,
    pub payment_terms_days: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SupplierBalance {
    pub supplier_id:     i64,
    pub name:            String,
    pub code:            Option<String>,
    pub total_purchases: f64,
    pub total_paid:      f64,
    pub balance:         f64,
    pub order_count:     i64,
    pub last_delivery:   Option<String>,
}

// ─── Supplier CRUD ────────────────────────────────────────────────────────────

#[command]
pub async fn cmd_get_suppliers(
    state: State<'_, AppState>,
) -> Result<Vec<Supplier>, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    let mut stmt = conn.prepare("
        SELECT id, code, name, name_ar, contact_name, phone, email,
               address, wilaya, nif, nis, rc,
               payment_terms_days, total_debt_dzd, is_active, created_at
        FROM suppliers
        ORDER BY name
    ").map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |r| Ok(Supplier {
        id:                 r.get(0)?,
        code:               r.get(1)?,
        name:               r.get(2)?,
        name_ar:            r.get(3)?,
        contact_name:       r.get(4)?,
        phone:              r.get(5)?,
        email:              r.get(6)?,
        address:            r.get(7)?,
        wilaya:             r.get(8)?,
        nif:                r.get(9)?,
        nis:                r.get(10)?,
        rc:                 r.get(11)?,
        payment_terms_days: r.get(12)?,
        total_debt_dzd:     r.get(13)?,
        is_active:          r.get::<_, i64>(14)? == 1,
        created_at:         r.get(15)?,
    })).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    Ok(rows)
}

#[command]
pub async fn cmd_create_supplier(
    state: State<'_, AppState>,
    input: CreateSupplierInput,
) -> Result<i64, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    conn.execute(
        "INSERT INTO suppliers
         (code, name, name_ar, contact_name, phone, email,
          address, wilaya, nif, nis, rc, payment_terms_days)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            input.code, input.name,
            input.name_ar.unwrap_or_default(),
            input.contact_name, input.phone, input.email,
            input.address, input.wilaya,
            input.nif, input.nis, input.rc,
            input.payment_terms_days.unwrap_or(30),
        ],
    ).map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

#[command]
pub async fn cmd_update_supplier(
    state: State<'_, AppState>,
    input: Supplier,
) -> Result<(), String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    conn.execute(
        "UPDATE suppliers
         SET code=?1, name=?2, name_ar=?3, contact_name=?4, phone=?5,
             email=?6, address=?7, wilaya=?8, nif=?9, nis=?10, rc=?11,
             payment_terms_days=?12, is_active=?13
         WHERE id=?14",
        params![
            input.code, input.name, input.name_ar, input.contact_name,
            input.phone, input.email, input.address, input.wilaya,
            input.nif, input.nis, input.rc,
            input.payment_terms_days,
            input.is_active as i64,
            input.id,
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// AP balances for all suppliers — uses v_supplier_balance view.
#[command]
pub async fn cmd_get_supplier_balances(
    state: State<'_, AppState>,
) -> Result<Vec<SupplierBalance>, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    let mut stmt = conn.prepare("
        SELECT supplier_id, name, code,
               total_purchases, total_paid, balance,
               order_count, last_delivery
        FROM v_supplier_balance
        WHERE is_active = 1
        ORDER BY balance DESC
    ").map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |r| Ok(SupplierBalance {
        supplier_id:     r.get(0)?,
        name:            r.get(1)?,
        code:            r.get(2)?,
        total_purchases: r.get(3)?,
        total_paid:      r.get(4)?,
        balance:         r.get(5)?,
        order_count:     r.get(6)?,
        last_delivery:   r.get(7)?,
    })).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    Ok(rows)
}

// ─── Purchase Orders ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PurchaseOrder {
    pub id:              i64,
    pub ref_number:      String,
    pub supplier_id:     i64,
    pub supplier_name:   String,
    pub status:          String,
    pub invoice_number:  Option<String>,
    pub total_ht:        f64,
    pub total_ttc:       f64,
    pub discount_amount: f64,
    pub amount_paid:     f64,
    pub notes:           Option<String>,
    pub received_at:     String,
    pub item_count:      i64,
}

#[derive(Debug, Deserialize)]
pub struct CreatePurchaseOrderInput {
    pub supplier_id:     i64,
    pub invoice_number:  Option<String>,
    pub notes:           Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PurchaseItem {
    pub id:                i64,
    pub purchase_order_id: i64,
    pub product_id:        i64,
    pub product_name:      String,
    pub gtin:              Option<String>,
    pub quantity_ordered:  f64,
    pub quantity_received: f64,
    pub cost_price_ht:     f64,
    pub batch_number:      Option<String>,
    pub expiry_date:       Option<String>,
    pub batch_id:          Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AddPurchaseItemInput {
    pub purchase_order_id: i64,
    pub product_id:        i64,
    pub quantity_ordered:  f64,
    pub quantity_received: f64,
    pub cost_price_ht:     f64,
    pub tax_rate_id:       Option<i64>,
    pub batch_number:      Option<String>,
    pub expiry_date:       Option<String>,
}

#[command]
pub async fn cmd_get_purchase_orders(
    state: State<'_, AppState>,
) -> Result<Vec<PurchaseOrder>, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    let mut stmt = conn.prepare("
        SELECT po.id, po.ref_number, po.supplier_id, s.name,
               po.status, po.invoice_number, po.total_ht, po.total_ttc,
               po.discount_amount, po.amount_paid, po.notes, po.received_at,
               (SELECT COUNT(*) FROM purchase_items WHERE purchase_order_id = po.id) AS items
        FROM purchase_orders po
        JOIN suppliers s ON s.id = po.supplier_id
        ORDER BY po.received_at DESC
        LIMIT 200
    ").map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |r| Ok(PurchaseOrder {
        id:              r.get(0)?,
        ref_number:      r.get(1)?,
        supplier_id:     r.get(2)?,
        supplier_name:   r.get(3)?,
        status:          r.get(4)?,
        invoice_number:  r.get(5)?,
        total_ht:        r.get(6)?,
        total_ttc:       r.get(7)?,
        discount_amount: r.get(8)?,
        amount_paid:     r.get(9)?,
        notes:           r.get(10)?,
        received_at:     r.get(11)?,
        item_count:      r.get(12)?,
    })).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    Ok(rows)
}

#[command]
pub async fn cmd_create_purchase_order(
    state: State<'_, AppState>,
    input: CreatePurchaseOrderInput,
) -> Result<i64, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    // Generate sequential reference number for today
    let seq: i64 = conn.query_row(
        "SELECT COUNT(*) + 1 FROM purchase_orders WHERE DATE(received_at) = DATE('now')",
        [],
        |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    let today = chrono::Utc::now().format("%Y%m%d").to_string();
    let ref_number = format!("BL-{today}-{seq:04}");

    conn.execute(
        "INSERT INTO purchase_orders (ref_number, supplier_id, invoice_number, notes)
         VALUES (?1, ?2, ?3, ?4)",
        params![ref_number, input.supplier_id, input.invoice_number, input.notes],
    ).map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

#[command]
pub async fn cmd_add_purchase_item(
    state: State<'_, AppState>,
    input: AddPurchaseItemInput,
) -> Result<i64, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    // This insert triggers trg_v2_create_batch_on_purchase automatically.
    conn.execute(
        "INSERT INTO purchase_items
         (purchase_order_id, product_id, quantity_ordered, quantity_received,
          cost_price_ht, tax_rate_id, batch_number, expiry_date)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            input.purchase_order_id,
            input.product_id,
            input.quantity_ordered,
            input.quantity_received,
            input.cost_price_ht,
            input.tax_rate_id,
            input.batch_number,
            input.expiry_date,
        ],
    ).map_err(|e| e.to_string())?;

    // Optionally update product buy_price to reflect new cost
    if input.cost_price_ht > 0.0 {
        conn.execute(
            "UPDATE products SET buy_price = ?1 WHERE id = ?2",
            params![input.cost_price_ht, input.product_id],
        ).map_err(|e| e.to_string())?;
    }

    Ok(conn.last_insert_rowid())
}

#[command]
pub async fn cmd_receive_purchase_order(
    state: State<'_, AppState>,
    id:    i64,
) -> Result<(), String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    conn.execute(
        "UPDATE purchase_orders SET status = 'invoiced', invoiced_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?1 AND status = 'received'",
        params![id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

// ─── Stock Adjustments ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct StockAdjustmentInput {
    pub batch_id:        i64,
    pub product_id:      i64,
    pub adjustment_type: String,
    pub quantity_delta:  f64,
    pub reason:          Option<String>,
    pub adjusted_by:     Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StockAdjustmentRow {
    pub id:              i64,
    pub batch_id:        i64,
    pub product_name:    String,
    pub adjustment_type: String,
    pub quantity_delta:  f64,
    pub quantity_before: f64,
    pub quantity_after:  f64,
    pub reason:          Option<String>,
    pub adjusted_by:     String,
    pub adjusted_at:     String,
}

#[command]
pub async fn cmd_create_stock_adjustment(
    state: State<'_, AppState>,
    input: StockAdjustmentInput,
) -> Result<i64, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    // Get current quantity
    let current_qty: f64 = conn.query_row(
        "SELECT quantity FROM inventory_batches WHERE id = ?1",
        params![input.batch_id],
        |r| r.get(0),
    ).map_err(|e| format!("Batch not found: {e}"))?;

    let new_qty = current_qty + input.quantity_delta;
    if new_qty < 0.0 {
        return Err(format!(
            "Ajustement invalide : le stock serait négatif ({current_qty:.2} + {:.2} = {new_qty:.2})",
            input.quantity_delta
        ));
    }

    conn.execute(
        "INSERT INTO stock_adjustments
         (batch_id, product_id, adjustment_type, quantity_delta,
          quantity_before, quantity_after, reason, adjusted_by)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            input.batch_id,
            input.product_id,
            input.adjustment_type,
            input.quantity_delta,
            current_qty,
            new_qty,
            input.reason,
            input.adjusted_by.unwrap_or_else(|| "Admin".into()),
        ],
    ).map_err(|e| e.to_string())?;

    // The trigger trg_v2_apply_adjustment updates inventory_batches.quantity.
    Ok(conn.last_insert_rowid())
}

#[command]
pub async fn cmd_get_stock_adjustments(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<StockAdjustmentRow>, String> {
    let db   = state.db.lock().unwrap();
    let conn = &db.0;

    let lim = limit.unwrap_or(200);
    let mut stmt = conn.prepare("
        SELECT sa.id, sa.batch_id, p.name_fr,
               sa.adjustment_type, sa.quantity_delta,
               sa.quantity_before, sa.quantity_after,
               sa.reason, sa.adjusted_by, sa.adjusted_at
        FROM stock_adjustments sa
        JOIN products p ON p.id = sa.product_id
        ORDER BY sa.adjusted_at DESC
        LIMIT ?1
    ").map_err(|e| e.to_string())?;

    let rows = stmt.query_map(params![lim], |r| Ok(StockAdjustmentRow {
        id:              r.get(0)?,
        batch_id:        r.get(1)?,
        product_name:    r.get(2)?,
        adjustment_type: r.get(3)?,
        quantity_delta:  r.get(4)?,
        quantity_before: r.get(5)?,
        quantity_after:  r.get(6)?,
        reason:          r.get(7)?,
        adjusted_by:     r.get(8)?,
        adjusted_at:     r.get(9)?,
    })).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    Ok(rows)
}