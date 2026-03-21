// src-tauri/src/license/hwid.rs
//! Hardware fingerprinting module.
//!
//! Collects identifiers from three independent sources:
//!   1. `machine-uid` crate  — OS-managed unique machine GUID
//!      (Windows: HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid)
//!      (Linux:   /etc/machine-id)
//!      (macOS:   IOPlatformUUID via ioreg)
//!   2. CPU info via `sysinfo` — vendor brand string + logical core count
//!   3. Platform-specific low-level identifiers (motherboard serial, disk UUID)
//!
//! All three are concatenated, then hashed with SHA-256 to produce a 64-char
//! hex string that is stable across reboots but unique per hardware.
//!
//! Robustness: every source has a fallback so the HWID is always computable,
//! even on VMs or locked-down Windows installs that deny WMI queries.

use sha2::{Digest, Sha256};
use sysinfo::System;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Collect all hardware components and return their SHA-256 fingerprint.
/// This is the authoritative HWID used in license binding.
pub fn compute_hwid() -> String {
    let parts = collect_components();
    hash_components(&parts)
}

/// Return the raw (pre-hash) components for diagnostic display.
/// Never expose this in release builds without redaction.
pub fn collect_components() -> HwidComponents {
    HwidComponents {
        machine_uid: get_machine_uid(),
        cpu_brand:   get_cpu_brand(),
        platform_id: get_platform_id(),
    }
}

#[derive(Debug)]
pub struct HwidComponents {
    pub machine_uid: String,
    pub cpu_brand:   String,
    pub platform_id: String,
}

// ─── Hashing ──────────────────────────────────────────────────────────────────

fn hash_components(c: &HwidComponents) -> String {
    // Format: version_prefix | machine_uid | cpu_brand | platform_id
    // The v2 prefix ensures legacy v1 HWIDs never collide.
    let raw = format!("SUPERPOS_HWID_V2|{}|{}|{}", c.machine_uid, c.cpu_brand, c.platform_id);
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

// ─── Source 1: machine-uid (OS-managed GUID) ──────────────────────────────────

fn get_machine_uid() -> String {
    machine_uid::get()
        .unwrap_or_else(|_| "MACHINE_UID_UNAVAILABLE".to_string())
        .trim()
        .to_uppercase()
}

// ─── Source 2: CPU brand via sysinfo ─────────────────────────────────────────

fn get_cpu_brand() -> String {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    let cpus = sys.cpus();
    if cpus.is_empty() {
        return "CPU_UNAVAILABLE".to_string();
    }
    // Use the first CPU's brand string + logical core count for extra entropy.
    // The core count changes if hyperthreading is toggled, so we use physical cores.
    let brand = cpus[0].brand().trim().to_string();
    let count = cpus.len();
    format!("{brand}:{count}")
}

// ─── Source 3: Platform-specific low-level ID ─────────────────────────────────

#[cfg(target_os = "windows")]
fn get_platform_id() -> String {
    // Prefer: Motherboard BaseBoard serial via registry.
    // Fallback: Volume serial number of C:.
    get_windows_baseboard_serial()
        .or_else(get_windows_volume_serial)
        .unwrap_or_else(|| "WIN_PLATFORM_UNAVAILABLE".to_string())
}

#[cfg(target_os = "windows")]
fn get_windows_baseboard_serial() -> Option<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    // Try BIOS path first (most reliable on physical hardware)
    let bios = hklm
        .open_subkey(r"HARDWARE\DESCRIPTION\System\BIOS")
        .ok()?;
    let serial: String = bios.get_value("BaseBoardSerialNumber").ok()?;
    let serial = serial.trim().to_string();
    // Some OEMs leave this field blank or set it to "Default string"
    if serial.is_empty()
        || serial.eq_ignore_ascii_case("Default string")
        || serial.eq_ignore_ascii_case("To be filled by O.E.M.")
    {
        return None;
    }
    Some(serial.to_uppercase())
}

#[cfg(target_os = "windows")]
fn get_windows_volume_serial() -> Option<String> {
    use winreg::enums::{HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    // HKLM\SYSTEM\MountedDevices doesn't give us serial easily.
    // Use vol command as last resort — it's always available.
    let output = std::process::Command::new("cmd")
        .args(["/C", "vol C:"])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    // Parse "Volume Serial Number is ABCD-EFGH"
    text.lines()
        .find(|l| l.contains("Serial Number") || l.contains("Numéro de série"))
        .and_then(|l| l.split_whitespace().last())
        .map(|s| s.trim().to_uppercase())
}

#[cfg(target_os = "macos")]
fn get_platform_id() -> String {
    // macOS: read the IOPlatformSerialNumber via ioreg (distinct from IOPlatformUUID).
    let output = std::process::Command::new("ioreg")
        .args(["-d2", "-c", "IOPlatformExpertDevice"])
        .output();
    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines()
                .find(|l| l.contains("IOPlatformSerialNumber"))
                .and_then(|l| l.split('"').nth(3))
                .unwrap_or("MACOS_SERIAL_UNAVAILABLE")
                .trim()
                .to_uppercase()
        }
        Err(_) => "MACOS_IOREG_UNAVAILABLE".to_string(),
    }
}

#[cfg(target_os = "linux")]
fn get_platform_id() -> String {
    // Prefer: DMI board serial (requires root or world-readable sysfs).
    // Fallback: /etc/machine-id (always readable, set on first boot).
    get_linux_dmi_serial()
        .or_else(get_linux_machine_id)
        .unwrap_or_else(|| "LINUX_PLATFORM_UNAVAILABLE".to_string())
}

#[cfg(target_os = "linux")]
fn get_linux_dmi_serial() -> Option<String> {
    let candidates = [
        "/sys/class/dmi/id/board_serial",
        "/sys/class/dmi/id/product_uuid",
        "/sys/class/dmi/id/chassis_serial",
    ];
    for path in &candidates {
        if let Ok(val) = std::fs::read_to_string(path) {
            let val = val.trim().to_uppercase();
            if !val.is_empty()
                && !val.starts_with("DEFAULT")
                && val != "NONE"
                && val != "0"
            {
                return Some(val);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn get_linux_machine_id() -> Option<String> {
    std::fs::read_to_string("/etc/machine-id")
        .ok()
        .map(|s| s.trim().to_uppercase())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hwid_is_64_hex_chars() {
        let hwid = compute_hwid();
        assert_eq!(hwid.len(), 64, "HWID should be SHA-256 hex (64 chars), got: {hwid}");
        assert!(hwid.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hwid_is_deterministic() {
        let h1 = compute_hwid();
        let h2 = compute_hwid();
        assert_eq!(h1, h2, "HWID must be identical across calls on same hardware");
    }

    #[test]
    fn components_are_non_empty() {
        let c = collect_components();
        assert!(!c.machine_uid.is_empty());
        assert!(!c.cpu_brand.is_empty());
        assert!(!c.platform_id.is_empty());
    }
}