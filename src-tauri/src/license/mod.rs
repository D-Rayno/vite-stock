// src-tauri/src/license/mod.rs
//! License module — public API consumed by Tauri commands and `main.rs`.
//!
//! Sub-modules:
//!   hwid   — hardware fingerprinting (multi-source, SHA-256 hashed)
//!   crypto — Ed25519 verification + AES-256-GCM encrypted storage

pub mod crypto;
pub mod hwid;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

// Re-export the HWID function so callers don't need to know the sub-module.
pub use hwid::compute_hwid;

// ─── Feature flags ────────────────────────────────────────────────────────────

pub mod features {
    #[allow(dead_code)]
    pub const POS_BASIC:          u32 = 1 << 0;
    #[allow(dead_code)]
    pub const INVENTORY_MGMT:     u32 = 1 << 1;
    pub const THERMAL_PRINT:      u32 = 1 << 2;
    pub const DAIN_LEDGER:        u32 = 1 << 3;
    pub const A4_REPORTS:         u32 = 1 << 4;
    #[allow(dead_code)]
    pub const MULTI_CART:         u32 = 1 << 5;
    #[allow(dead_code)]
    pub const ADVANCED_ANALYTICS: u32 = 1 << 6;

    #[allow(dead_code)]
    pub const TIER_BASIC:        u32 = POS_BASIC;
    #[allow(dead_code)]
    pub const TIER_PROFESSIONAL: u32 = POS_BASIC | INVENTORY_MGMT | THERMAL_PRINT;
    #[allow(dead_code)]
    pub const TIER_ENTERPRISE:   u32 = TIER_PROFESSIONAL
        | DAIN_LEDGER | A4_REPORTS | MULTI_CART | ADVANCED_ANALYTICS;
}

// ─── Runtime state ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LicenseState {
    pub is_valid:   bool,
    pub tier:       String,
    pub features:   u32,
    pub expires_at: Option<String>,
    /// Human-readable rejection reason shown to the user when `is_valid = false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection:  Option<String>,
}

impl LicenseState {
    pub fn has_feature(&self, flag: u32) -> bool {
        self.is_valid && (self.features & flag) != 0
    }

    fn rejected(reason: impl Into<String>) -> Self {
        LicenseState {
            is_valid:  false,
            rejection: Some(reason.into()),
            ..Default::default()
        }
    }
}

// ─── License payload (inside the signed JWT-like blob) ───────────────────────

/// The JSON payload that the vendor signs with their Ed25519 private key.
/// Fields are sorted alphabetically when serialised so the signature is stable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicensePayload {
    pub expires_at:   Option<String>,   // ISO-8601 date or null
    pub features:     u32,
    pub hwid:         String,           // SHA-256 hex of machine fingerprint
    pub issued_at:    String,           // ISO-8601 date
    pub machine_name: Option<String>,   // optional label for the client
    pub tier:         String,           // basic | professional | enterprise
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum LicenseError {
    #[error("No license file found")]
    NotFound,

    #[error("License file is corrupt or has been tampered with")]
    Corrupt,

    #[error("License signature is invalid — key was not issued by SuperPOS")]
    SignatureInvalid,

    #[error("License is bound to a different machine (HWID mismatch)")]
    HwidMismatch,

    #[error("License has expired on {0}")]
    Expired(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl LicenseError {
    /// User-friendly French message for display in the activation screen.
    pub fn user_message(&self) -> &str {
        match self {
            LicenseError::NotFound       => "Aucune licence trouvée. Veuillez activer le logiciel.",
            LicenseError::Corrupt        => "Fichier de licence corrompu. Réactivez le logiciel.",
            LicenseError::SignatureInvalid => "Clé de licence invalide. Vérifiez la clé fournie.",
            LicenseError::HwidMismatch   => "Cette licence est liée à un autre appareil.",
            LicenseError::Expired(_)     => "Votre licence a expiré. Contactez votre revendeur.",
            LicenseError::StorageError(_)=> "Erreur de stockage de la licence.",
            LicenseError::Io(_)          => "Erreur d'accès au fichier de licence.",
        }
    }
}

// ─── Disk persistence ─────────────────────────────────────────────────────────

/// Encrypted license file — binary format (nonce || ciphertext).
const LICENSE_FILE: &str = "superpos.lic";

/// Load the license from the encrypted on-disk file.
/// On success, returns a fully validated `LicenseState`.
/// On any error, returns a `LicenseState { is_valid: false, rejection: ... }`.
pub fn load_from_disk(app_dir: &Path) -> LicenseState {
    match _load_from_disk(app_dir) {
        Ok(state) => state,
        Err(e)    => LicenseState::rejected(e.user_message()),
    }
}

fn _load_from_disk(app_dir: &Path) -> Result<LicenseState, LicenseError> {
    let path = app_dir.join(LICENSE_FILE);
    if !path.exists() { return Err(LicenseError::NotFound); }

    // 1. Read encrypted bytes
    let encrypted = std::fs::read(&path)?;

    // 2. Decrypt with machine-derived key
    let machine_uid = machine_uid::get()
        .unwrap_or_else(|_| "MACHINE_UID_FALLBACK".into());
    let license_key = crypto::decrypt_from_storage(&encrypted, &machine_uid)
        .map_err(|_| LicenseError::StorageError(
            "decrypt failed — file may be from another machine".into()
        ))?;

    // 3. Parse + verify signature + validate payload
    verify_and_build_state(&license_key)
}

/// Save a validated license key string to disk (encrypted).
pub fn save_to_disk(app_dir: &Path, license_key: &str) -> Result<(), LicenseError> {
    let machine_uid = machine_uid::get()
        .unwrap_or_else(|_| "MACHINE_UID_FALLBACK".into());
    let encrypted = crypto::encrypt_for_storage(license_key, &machine_uid)?;
    let path = app_dir.join(LICENSE_FILE);
    std::fs::write(path, encrypted)?;
    Ok(())
}

// ─── Core verification pipeline ───────────────────────────────────────────────

/// Full verification pipeline:
///   parse wire format → verify Ed25519 signature → check HWID → check expiry
/// Returns `LicenseState` on success.
pub fn verify_and_build_state(license_key: &str) -> Result<LicenseState, LicenseError> {
    // Step 1 — Parse wire format
    let (payload_bytes, signature) = crypto::parse_license_key(license_key)?;

    // Step 2 — Verify Ed25519 signature
    // This is the critical check. Without the vendor's private key, this
    // cannot be forged regardless of what an attacker puts in the payload.
    crypto::verify_signature(&payload_bytes, &signature)?;

    // Step 3 — Deserialise payload
    let payload: LicensePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| LicenseError::Corrupt)?;

    // Step 4 — Verify HWID binding
    let local_hwid = compute_hwid();
    if payload.hwid != local_hwid {
        return Err(LicenseError::HwidMismatch);
    }

    // Step 5 — Check expiry
    if let Some(ref exp) = payload.expires_at {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        if exp.as_str() < today.as_str() {
            return Err(LicenseError::Expired(exp.clone()));
        }
    }

    Ok(LicenseState {
        is_valid:   true,
        tier:       payload.tier,
        features:   payload.features,
        expires_at: payload.expires_at,
        rejection:  None,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_prefix_fails() {
        let err = verify_and_build_state("INVALID-abc.def");
        assert!(err.is_err());
        assert!(matches!(err.unwrap_err(), LicenseError::Corrupt));
    }

    #[test]
    fn tampered_payload_fails_signature() {
        // Valid structure but fake data — signature won't match
        let fake = "SUPERPOS-dGhpcyBpcyBmYWtl.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let err = verify_and_build_state(fake);
        assert!(err.is_err());
    }
}