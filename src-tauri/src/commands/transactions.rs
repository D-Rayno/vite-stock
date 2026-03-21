// src-tauri/src/commands/transactions.rs
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{command, State};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct TransactionItemInput {
    pub product_id:   i64,
    pub batch_id:     Option<i64>,
    pub quantity:     f64,
    pub unit_price:   f64,
    pub vat_rate:     f64,
    pub discount_pct: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTransactionInput {
    pub customer_id:     Option<i64>,
    pub items:           Vec<TransactionItemInput>,
    pub discount_amount: Option<f64>,
    pub payment_method:  String,
    pub amount_paid:     f64,
    pub cashier_name:    String,
    pub notes:           Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TransactionSummary {
    pub id:             i64,
    pub ref_number:     String,
    pub total_ttc:      f64,
    pub change_given:   f64,
}

#[command]
pub async fn cmd_create_transaction(
    state: State<'_, AppState>,
    input: CreateTransactionInput,
) -> Result<TransactionSummary, String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;

    // Calculate totals.
    let discount = input.discount_amount.unwrap_or(0.0);
    let total_ht: f64 = input.items.iter().map(|i| {
        let line = i.unit_price * i.quantity;
        let disc = line * (i.discount_pct.unwrap_or(0.0) / 100.0);
        line - disc
    }).sum();
    let total_ttc: f64 = input.items.iter().map(|i| {
        let line = i.unit_price * i.quantity * (1.0 + i.vat_rate);
        let disc = line * (i.discount_pct.unwrap_or(0.0) / 100.0);
        line - disc
    }).sum::<f64>() - discount;

    let change = (input.amount_paid - total_ttc).max(0.0);

    // Generate a sequential reference number.
    let seq: i64 = conn.query_row(
        "SELECT COUNT(*) + 1 FROM transactions WHERE DATE(created_at) = DATE('now')",
        [],
        |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    let today = chrono::Utc::now().format("%Y%m%d").to_string();
    let ref_number = format!("TXN-{today}-{seq:04}");

    conn.execute(
        "INSERT INTO transactions (ref_number, customer_id, total_ttc, total_ht,
                                   discount_amount, payment_method, amount_paid,
                                   change_given, cashier_name, notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            ref_number, input.customer_id, total_ttc, total_ht,
            discount, input.payment_method, input.amount_paid,
            change, input.cashier_name, input.notes,
        ],
    ).map_err(|e| e.to_string())?;

    let txn_id = conn.last_insert_rowid();

    for item in &input.items {
        conn.execute(
            "INSERT INTO transaction_items (transaction_id, product_id, batch_id,
                                            quantity, unit_price, vat_rate, discount_pct)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                txn_id, item.product_id, item.batch_id,
                item.quantity, item.unit_price, item.vat_rate,
                item.discount_pct.unwrap_or(0.0),
            ],
        ).map_err(|e| e.to_string())?;
    }

    Ok(TransactionSummary { id: txn_id, ref_number, total_ttc, change_given: change })
}

#[command]
pub async fn cmd_get_transaction(
    state: State<'_, AppState>,
    id: i64,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;

    let txn = conn.query_row(
        "SELECT id, ref_number, customer_id, total_ttc, total_ht,
                discount_amount, payment_method, amount_paid, change_given,
                cashier_name, notes, created_at
         FROM transactions WHERE id=?1",
        params![id],
        |r| Ok(serde_json::json!({
            "id":             r.get::<_,i64>(0)?,
            "ref_number":     r.get::<_,String>(1)?,
            "customer_id":    r.get::<_,Option<i64>>(2)?,
            "total_ttc":      r.get::<_,f64>(3)?,
            "total_ht":       r.get::<_,f64>(4)?,
            "discount_amount":r.get::<_,f64>(5)?,
            "payment_method": r.get::<_,String>(6)?,
            "amount_paid":    r.get::<_,f64>(7)?,
            "change_given":   r.get::<_,f64>(8)?,
            "cashier_name":   r.get::<_,String>(9)?,
            "notes":          r.get::<_,Option<String>>(10)?,
            "created_at":     r.get::<_,String>(11)?,
        }))
    ).map_err(|e| e.to_string())?;

    Ok(txn)
}