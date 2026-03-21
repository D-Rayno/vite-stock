// src-tauri/src/commands/settings.rs
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{command, State};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppSettings(pub HashMap<String, String>);

#[command]
pub async fn cmd_get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;

    let mut stmt = conn.prepare("SELECT key, value FROM settings")
        .map_err(|e| e.to_string())?;

    let map: HashMap<String, String> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(AppSettings(map))
}

#[command]
pub async fn cmd_update_settings(
    state: State<'_, AppState>,
    updates: HashMap<String, String>,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    let conn = &db.0;

    for (key, value) in &updates {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value,
             updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",
            params![key, value],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}