// src-tauri/src/bin/keygen.rs
//! SuperPOS License Key Generator — vendor-side CLI.
//!
//! This binary is NEVER distributed to customers.
//! It is compiled and run only on the vendor's secure machine.
//!
//! Commands:
//!   gen-keypair                        Generate a new Ed25519 keypair
//!   issue --key <private.key>          Issue a license for a HWID
//!         --hwid <64-char hex>
//!         --tier <basic|professional|enterprise>
//!         [--expires YYYY-MM-DD]
//!         [--label "Shop Name"]
//!   verify --key <private.key> <license-key>   Verify a previously issued key

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use chrono::Utc;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::Serialize;
use sha2::{Digest, Sha256};

// ─── Embedded feature flags (mirrors license/mod.rs) ─────────────────────────

mod features {
    pub const POS_BASIC:          u32 = 1 << 0;
    pub const INVENTORY_MGMT:     u32 = 1 << 1;
    pub const THERMAL_PRINT:      u32 = 1 << 2;
    pub const DAIN_LEDGER:        u32 = 1 << 3;
    pub const A4_REPORTS:         u32 = 1 << 4;
    pub const MULTI_CART:         u32 = 1 << 5;
    pub const ADVANCED_ANALYTICS: u32 = 1 << 6;

    pub const TIER_BASIC:        u32 = POS_BASIC;
    pub const TIER_PROFESSIONAL: u32 = POS_BASIC | INVENTORY_MGMT | THERMAL_PRINT;
    pub const TIER_ENTERPRISE:   u32 = TIER_PROFESSIONAL
        | DAIN_LEDGER | A4_REPORTS | MULTI_CART | ADVANCED_ANALYTICS;

    pub fn from_tier(tier: &str) -> u32 {
        match tier {
            "basic"        => TIER_BASIC,
            "professional" => TIER_PROFESSIONAL,
            "enterprise"   => TIER_ENTERPRISE,
            _              => TIER_BASIC,
        }
    }
}

// ─── Payload ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct LicensePayload {
    expires_at:   Option<String>,
    features:     u32,
    hwid:         String,
    issued_at:    String,
    machine_name: Option<String>,
    tier:         String,
}

// ─── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("gen-keypair") => cmd_gen_keypair(),
        Some("issue")       => cmd_issue(&args[2..]),
        Some("verify")      => cmd_verify(&args[2..]),
        Some("help") | None => print_help(),
        Some(cmd)           => {
            eprintln!("Unknown command: {cmd}");
            print_help();
            std::process::exit(1);
        }
    }
}

// ─── gen-keypair ──────────────────────────────────────────────────────────────

fn cmd_gen_keypair() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key: VerifyingKey = signing_key.verifying_key();

    let private_hex = hex::encode(signing_key.to_bytes());
    let public_hex  = hex::encode(verifying_key.to_bytes());

    // XOR with a random mask for embedding in the binary
    let mask: [u8; 32] = {
        use rand::RngCore;
        let mut m = [0u8; 32];
        OsRng.fill_bytes(&mut m);
        m
    };

    let obfuscated: Vec<u8> = verifying_key.to_bytes()
        .iter()
        .zip(mask.iter())
        .map(|(a, b)| a ^ b)
        .collect();

    println!("══════════════════════════════════════════════════════════════");
    println!("  SuperPOS License Keypair");
    println!("══════════════════════════════════════════════════════════════");
    println!();
    println!("PRIVATE KEY (keep secret — never commit or distribute):");
    println!("  {private_hex}");
    println!();
    println!("PUBLIC KEY (plain hex — for reference):");
    println!("  {public_hex}");
    println!();
    println!("Paste the following into src/license/crypto.rs:");
    println!();
    println!("const OBFUSCATED_PUBKEY: [u8; 32] = [");
    print_hex_array(&obfuscated);
    println!("];");
    println!();
    println!("const XOR_MASK: [u8; 32] = [");
    print_hex_array(&mask);
    println!("];");
    println!();
    println!("⚠  Save the PRIVATE KEY in a password manager — if lost, you");
    println!("   cannot issue new licenses. Re-run gen-keypair to rotate.");
}

fn print_hex_array(bytes: &[u8]) {
    let rows: Vec<String> = bytes.chunks(8).map(|chunk| {
        let cells: Vec<String> = chunk.iter().map(|b| format!("0x{b:02x}")).collect();
        format!("    {}", cells.join(", "))
    }).collect();
    println!("{},", rows.join(",\n"));
}

// ─── issue ────────────────────────────────────────────────────────────────────

fn cmd_issue(args: &[String]) {
    let private_key_path = require_flag(args, "--key");
    let hwid             = require_flag(args, "--hwid");
    let tier             = flag(args, "--tier").unwrap_or_else(|| "basic".into());
    let expires          = flag(args, "--expires");
    let label            = flag(args, "--label");

    // Validate HWID format
    if hwid.len() != 64 || !hwid.chars().all(|c| c.is_ascii_hexdigit()) {
        eprintln!("ERROR: HWID must be a 64-character hex string.");
        std::process::exit(1);
    }

    // Load private key
    let signing_key = load_signing_key(&private_key_path);

    let features = features::from_tier(&tier);

    let payload = LicensePayload {
        hwid:         hwid.clone(),
        tier:         tier.clone(),
        features,
        issued_at:    Utc::now().format("%Y-%m-%d").to_string(),
        expires_at:   expires.clone(),
        machine_name: label.clone(),
    };

    // Canonical serialisation (serde_json sorts fields alphabetically by default
    // since our struct derives Serialize with fields in alphabetical order)
    let payload_json = serde_json::to_vec(&payload)
        .expect("payload serialisation should not fail");

    // Sign SHA-256(payload)
    let digest    = Sha256::digest(&payload_json);
    let signature = {
        use ed25519_dalek::Signer;
        signing_key.sign(&digest)
    };

    // Build license key string
    let payload_b64 = B64.encode(&payload_json);
    let sig_b64     = B64.encode(signature.to_bytes());
    let license_key = format!("SUPERPOS-{payload_b64}.{sig_b64}");

    println!("══════════════════════════════════════════════════════════════");
    println!("  SuperPOS License Key");
    println!("══════════════════════════════════════════════════════════════");
    println!("  HWID  : {hwid}");
    println!("  Tier  : {tier}");
    println!("  Feats : 0b{features:07b} ({})", features);
    println!("  Issued: {}", payload.issued_at);
    println!("  Expiry: {}", expires.as_deref().unwrap_or("perpetual"));
    if let Some(ref l) = label { println!("  Label : {l}"); }
    println!();
    println!("LICENSE KEY (give this to the customer):");
    println!();
    println!("{license_key}");
    println!();

    // Optionally write to file
    let outfile = PathBuf::from(format!("license_{}.key", &hwid[..12]));
    std::fs::write(&outfile, &license_key)
        .expect("could not write license file");
    println!("Saved to: {}", outfile.display());
}

// ─── verify ───────────────────────────────────────────────────────────────────

fn cmd_verify(args: &[String]) {
    // Last positional arg is the license key string (or a file path)
    let license_key_raw = args.last().cloned().unwrap_or_else(|| {
        eprintln!("ERROR: provide the license key as the last argument.");
        std::process::exit(1);
    });

    // Accept either a raw key or a file path
    let license_key = if std::path::Path::new(&license_key_raw).exists() {
        std::fs::read_to_string(&license_key_raw)
            .expect("could not read license file")
            .trim()
            .to_string()
    } else {
        license_key_raw
    };

    let private_key_path = require_flag(args, "--key");
    let signing_key = load_signing_key(&private_key_path);
    let verifying_key: VerifyingKey = signing_key.verifying_key();

    // Parse
    let body = match license_key.strip_prefix("SUPERPOS-") {
        Some(b) => b,
        None    => { eprintln!("ERROR: not a SuperPOS license key."); std::process::exit(1); }
    };

    let (payload_b64, sig_b64) = match body.split_once('.') {
        Some(p) => p,
        None    => { eprintln!("ERROR: malformed key (missing dot separator)."); std::process::exit(1); }
    };

    let payload_bytes = B64.decode(payload_b64).expect("bad payload base64");
    let sig_bytes     = B64.decode(sig_b64).expect("bad signature base64");
    let sig_arr: [u8; 64] = sig_bytes.try_into().expect("signature must be 64 bytes");
    let signature = ed25519_dalek::Signature::from_bytes(&sig_arr);

    let digest = Sha256::digest(&payload_bytes);
    use ed25519_dalek::Verifier;
    match verifying_key.verify_strict(&digest, &signature) {
        Ok(_) => {
            let payload: serde_json::Value = serde_json::from_slice(&payload_bytes)
                .expect("bad payload JSON");
            println!("✅ Signature VALID");
            println!("{}", serde_json::to_string_pretty(&payload).unwrap());
        }
        Err(e) => {
            eprintln!("❌ Signature INVALID: {e}");
            std::process::exit(2);
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn load_signing_key(path: &str) -> SigningKey {
    let hex_str = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("could not read private key from: {path}"));
    let bytes = hex::decode(hex_str.trim())
        .expect("private key file must contain 64 hex chars");
    let arr: [u8; 32] = bytes.try_into()
        .expect("private key must be 32 bytes");
    SigningKey::from_bytes(&arr)
}

fn require_flag(args: &[String], flag: &str) -> String {
    flag_pos(args, flag)
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| {
            eprintln!("ERROR: missing required flag: {flag}");
            std::process::exit(1);
        })
}

fn flag(args: &[String], name: &str) -> Option<String> {
    flag_pos(args, name).and_then(|i| args.get(i + 1).cloned())
}

fn flag_pos(args: &[String], name: &str) -> Option<usize> {
    args.iter().position(|a| a == name)
}

fn print_help() {
    println!(
r#"SuperPOS keygen — offline license key generator

USAGE:
  keygen gen-keypair
    Generate a new Ed25519 keypair and print embedding constants.

  keygen issue --key <private.hex> --hwid <64-hex> --tier <tier> [opts]
    Issue a new license key.
    --tier     basic | professional | enterprise  (default: basic)
    --expires  YYYY-MM-DD  (omit for perpetual)
    --label    "Customer shop name"

  keygen verify --key <private.hex> <license-key-or-file>
    Verify a previously issued license key.

EXAMPLE:
  keygen gen-keypair > keypair.txt
  keygen issue --key vendor.hex \
               --hwid a3f1... \
               --tier enterprise \
               --expires 2026-12-31 \
               --label "Supermarché Alger Centre"
"#
    );
}