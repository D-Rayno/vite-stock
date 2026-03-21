// src-tauri/src/commands/license.rs
//! Tauri commands for the license / activation system.
//! 
//! Security contract:
//!   • No command returns raw bytes or internal keys.
//!   • All validation happens in Rust — the frontend only receives
//!     a `LicenseState` struct and can never bypass checks by calling
//!     a "validate" command and ignoring the result.
//!   • `cmd_verify_license` (the main activation endpoint) persists the
//!     encrypted license to disk on success so it survives reboots.

use tauri::{command, AppHandle, Manager, State};

use crate::license::{
    self, load_from_disk, save_to_disk, verify_and_build_state, LicenseState,
};
use crate::AppState;

// ─── cmd_verify_license ───────────────────────────────────────────────────────
//
// Main activation command. Called when the user pastes a license key.
//
// Pipeline (all in Rust):
//   1. Parse wire format   (SUPERPOS-<b64url_payload>.<b64url_sig>)
//   2. Verify Ed25519 sig  (reject if vendor didn't sign it)
//   3. Check HWID binding  (reject if for a different machine)
//   4. Check expiry date   (reject if expired)
//   5. Encrypt and save to superpos.lic via AES-256-GCM(HKDF(machine_uid))
//   6. Update AppState and return new LicenseState

#[command]
pub async fn cmd_verify_license(
    app:   AppHandle,
    state: State<'_, AppState>,
    key:   String,
) -> Result<LicenseState, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;

    // Full validation pipeline
    let new_state = verify_and_build_state(key.trim())
        .map_err(|e| e.to_string())?;

    // Persist encrypted to disk
    save_to_disk(&app_dir, key.trim())
        .map_err(|e| format!("Storage error: {e}"))?;

    // Update global AppState
    *state.license.lock().unwrap() = new_state.clone();

    Ok(new_state)
}

// ─── cmd_get_license_state ────────────────────────────────────────────────────
//
// Called on every app launch. Returns the current license state from RAM
// (already validated during setup → no disk I/O here).

#[command]
pub async fn cmd_get_license_state(
    state: State<'_, AppState>,
) -> Result<LicenseState, String> {
    Ok(state.license.lock().unwrap().clone())
}

// ─── cmd_get_hwid ─────────────────────────────────────────────────────────────
//
// Returns the local machine's HWID fingerprint so the operator can
// purchase a license from the vendor.
// The HWID is a SHA-256 hex string — safe to share; it cannot be reversed.

#[command]
pub async fn cmd_get_hwid() -> Result<String, String> {
    Ok(license::compute_hwid())
}

// ─── cmd_get_hwid_components ──────────────────────────────────────────────────
//
// Returns the individual hardware components (for diagnostic display in the
// activation screen). Values are partially redacted in release builds.

#[command]
pub async fn cmd_get_hwid_components() -> Result<HwidDiagnostics, String> {
    let components = license::hwid::collect_components();

    Ok(HwidDiagnostics {
        hwid:        license::compute_hwid(),
        machine_uid: redact(&components.machine_uid),
        cpu_brand:   components.cpu_brand.clone(),
        platform_id: redact(&components.platform_id),
    })
}

#[derive(serde::Serialize)]
pub struct HwidDiagnostics {
    pub hwid:        String,
    pub machine_uid: String,
    pub cpu_brand:   String,
    pub platform_id: String,
}

/// Redact all but the last 8 characters of a string for display.
fn redact(s: &str) -> String {
    let visible = 8;
    if s.len() <= visible {
        return "*".repeat(s.len());
    }
    let mask = "*".repeat(s.len() - visible);
    format!("{mask}{}", &s[s.len() - visible..])
}

// ─── cmd_reload_license ───────────────────────────────────────────────────────
//
// Re-reads the encrypted license file from disk and revalidates it.
// Used after OS reboot or if the user reports activation issues.

#[command]
pub async fn cmd_reload_license(
    app:   AppHandle,
    state: State<'_, AppState>,
) -> Result<LicenseState, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    let new_state = load_from_disk(&app_dir);
    *state.license.lock().unwrap() = new_state.clone();
    Ok(new_state)
}