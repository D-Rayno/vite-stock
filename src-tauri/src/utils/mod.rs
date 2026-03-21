// src-tauri/src/utils/mod.rs
pub mod escpos;

use chrono::Utc;

pub fn today_iso() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

pub fn now_display() -> String {
    Utc::now().format("%d/%m/%Y %H:%M").to_string()
}

pub fn make_ref(seq: i64) -> String {
    let date = Utc::now().format("%Y%m%d").to_string();
    format!("TXN-{date}-{seq:04}")
}

pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}