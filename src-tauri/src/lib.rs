// src-tauri/src/lib.rs
//
// Tauri v2 convention: ALL app setup lives in lib.rs → exported as pub fn run().
// main.rs is a 3-line thin caller so mobile entry points can call this directly.

mod commands;
mod db;
mod license;
mod utils;

use std::sync::Mutex;
use tauri::Manager;
use db::DbConnection;
use license::LicenseState;

// ─── Shared application state ─────────────────────────────────────────────────
//
// Wrapped in Mutex so Tauri can hand it to async commands from multiple threads.
// `DbConnection` wraps rusqlite::Connection (which is not Send) behind a Mutex,
// so all database access is serialised — no concurrent writes can corrupt WAL.

pub struct AppState {
    pub db:      Mutex<DbConnection>,
    pub license: Mutex<LicenseState>,
}

// ─── App entry point ──────────────────────────────────────────────────────────

pub fn run() {
    tauri::Builder::default()
        // ── Plugins ──────────────────────────────────────────────────────────
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        // ── Setup hook: open DB + load license ───────────────────────────────
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&app_dir)?;

            let conn    = db::open_and_migrate(&app_dir.join("superpos.db"))
                .expect("SQLite init failed");
            let license = license::load_from_disk(&app_dir);

            app.manage(AppState {
                db:      Mutex::new(conn),
                license: Mutex::new(license),
            });
            Ok(())
        })
        // ── Tauri IPC command registry ────────────────────────────────────────
        .invoke_handler(tauri::generate_handler![
            // ── License ──────────────────────────────────────────────────
            commands::license::cmd_verify_license,
            commands::license::cmd_get_license_state,
            commands::license::cmd_get_hwid,
            commands::license::cmd_get_hwid_components,
            commands::license::cmd_reload_license,
            // ── Products ─────────────────────────────────────────────────
            commands::products::cmd_lookup_product,
            commands::products::cmd_get_products,
            commands::products::cmd_create_product,
            commands::products::cmd_update_product,
            commands::products::cmd_delete_product,
            commands::products::cmd_search_products,
            // ── Inventory ────────────────────────────────────────────────
            commands::inventory::cmd_get_inventory_batches,
            commands::inventory::cmd_add_inventory_batch,
            commands::inventory::cmd_get_expiry_alerts,
            // ── Transactions ─────────────────────────────────────────────
            commands::transactions::cmd_create_transaction,
            commands::transactions::cmd_get_transaction,
            // ── Reports ──────────────────────────────────────────────────
            commands::reports::cmd_get_daily_report,
            commands::reports::cmd_get_full_report,
            // ── Dain ─────────────────────────────────────────────────────
            commands::dain::cmd_get_customer,
            commands::dain::cmd_add_dain_entry,
            commands::dain::cmd_repay_dain,
            commands::dain::cmd_get_dain_history,
            // ── Suppliers ────────────────────────────────────────────────
            commands::suppliers::cmd_get_suppliers,
            commands::suppliers::cmd_create_supplier,
            commands::suppliers::cmd_update_supplier,
            commands::suppliers::cmd_get_supplier_balances,
            commands::suppliers::cmd_get_purchase_orders,
            commands::suppliers::cmd_create_purchase_order,
            commands::suppliers::cmd_add_purchase_item,
            commands::suppliers::cmd_receive_purchase_order,
            commands::suppliers::cmd_create_stock_adjustment,
            commands::suppliers::cmd_get_stock_adjustments,
            // ── Sessions ─────────────────────────────────────────────────
            commands::sessions::cmd_open_session,
            commands::sessions::cmd_close_session,
            commands::sessions::cmd_get_active_session,
            commands::sessions::cmd_list_sessions,
            // ── Printing ─────────────────────────────────────────────────
            commands::printing::cmd_list_printers,
            commands::printing::cmd_print_thermal_receipt,
            commands::printing::cmd_print_test_page,
            // ── Backup / Export ───────────────────────────────────────────
            commands::backup::cmd_create_backup,
            commands::backup::cmd_list_backups,
            commands::backup::cmd_export_sales_excel,
            // ── Alerts ───────────────────────────────────────────────────
            commands::alerts::cmd_run_startup_checks,
            // ── Settings ─────────────────────────────────────────────────
            commands::settings::cmd_get_settings,
            commands::settings::cmd_update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("SuperPOS startup failed");
}