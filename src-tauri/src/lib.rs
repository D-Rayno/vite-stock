// src-tauri/src/lib.rs
//! Tauri v2 application bootstrap.
//!
//! ## Phase 4 additions
//!
//! 1. **Background alert thread** — spawned after setup via `background::spawn()`.
//!    Polls the DB every N hours and fires native Windows Toast notifications for
//!    expiring products and low stock.
//!
//! 2. **Exit-time auto-backup** — hooks into `tauri::RunEvent::ExitRequested`.
//!    Performs a synchronous `VACUUM INTO` backup before the process terminates.
//!    The backup is quick (< 2s for typical POS databases) and transparent to
//!    the operator.
//!
//! 3. **Network printing + A4 PDF commands** registered in the IPC handler.

mod background;
mod commands;
mod db;
pub mod license;
mod utils;

use std::sync::Mutex;
use tauri::Manager;
use db::DbConnection;
use license::LicenseState;

// ─── Shared application state ─────────────────────────────────────────────────

pub struct AppState {
    pub db:      Mutex<DbConnection>,
    pub license: Mutex<LicenseState>,
}

// ─── App entry point ──────────────────────────────────────────────────────────

pub fn run() {
    let app = tauri::Builder::default()
        // ── Plugins ──────────────────────────────────────────────────────────
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        // ── Setup: open DB + load license + spawn background thread ──────────
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

            // ── Phase 4: spawn background alert poller ────────────────────────
            // The handle is cloned so the thread owns it independently.
            background::spawn(app.handle().clone());

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
            // ── Thermal Printing (Phase 4: serial + network) ──────────────
            commands::printing::cmd_list_printers,
            commands::printing::cmd_test_network_printer,
            commands::printing::cmd_print_thermal_receipt,
            commands::printing::cmd_print_thermal_dain_statement,
            commands::printing::cmd_print_test_page,
            // ── A4 PDF Export (Phase 4, Enterprise gate) ──────────────────
            commands::a4print::cmd_export_dain_pdf,
            commands::a4print::cmd_export_stock_pdf,
            commands::a4print::cmd_export_sales_pdf,
            // ── Backup / Excel Export ─────────────────────────────────────
            commands::backup::cmd_create_backup,
            commands::backup::cmd_list_backups,
            commands::backup::cmd_export_sales_excel,
            // ── Alerts (startup check) ────────────────────────────────────
            commands::alerts::cmd_run_startup_checks,
            // ── Settings ─────────────────────────────────────────────────
            commands::settings::cmd_get_settings,
            commands::settings::cmd_update_settings,
        ])
        .build(tauri::generate_context!())
        .expect("SuperPOS startup failed");

    // ── Phase 4: exit-time auto-backup ────────────────────────────────────────
    //
    // `RunEvent::ExitRequested` fires when all windows have been closed and the
    // runtime is about to terminate.  We perform a synchronous VACUUM INTO
    // backup here — rusqlite blocks until the file is written, then we return
    // and the process exits normally.
    //
    // We deliberately do NOT call `api.prevent_exit()` — the backup is quick
    // and we want the process to exit cleanly after it completes.
    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            perform_exit_backup(app_handle);
        }
    });
}

// ─── Exit-time backup ─────────────────────────────────────────────────────────

fn perform_exit_backup(app: &tauri::AppHandle) {
    // Resolve app data directory synchronously.
    let app_dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(_) => return,   // can't resolve — skip silently
    };

    let backup_dir = app_dir.join("backups");
    if let Err(e) = std::fs::create_dir_all(&backup_dir) {
        eprintln!("[SuperPOS] Exit backup: cannot create dir: {e}");
        return;
    }

    let ts   = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let dest = backup_dir.join(format!("superpos_exit_{ts}.db"));

    // Hold the DB mutex only for the VACUUM INTO call.
    let state = app.state::<AppState>();
    let db    = match state.db.lock() {
        Ok(g)  => g,
        Err(_) => return,   // poisoned — skip
    };

    let vacuum_sql = format!("VACUUM INTO '{}'", dest.to_string_lossy());
    match db.0.execute_batch(&vacuum_sql) {
        Ok(_)  => eprintln!("[SuperPOS] Exit backup saved to {}", dest.display()),
        Err(e) => eprintln!("[SuperPOS] Exit backup failed: {e}"),
    }

    // Drop the lock before returning so the OS can cleanly close the DB file.
    drop(db);

    // Prune: keep at most 30 exit backups (separate from the manual backups).
    prune_backups(&backup_dir, "superpos_exit_", 10);
}

fn prune_backups(dir: &std::path::Path, prefix: &str, keep: usize) {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(prefix)
        })
        .map(|e| e.path())
        .collect();

    entries.sort();   // ascending = oldest first (timestamp in name)

    while entries.len() > keep {
        if let Some(oldest) = entries.first() {
            let _ = std::fs::remove_file(oldest);
            entries.remove(0);
        }
    }
}