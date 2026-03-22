// src-tauri/src/license/crypto.rs
//!
//! Cryptographic primitives for the license system.
//!
//! ## Trust model
//!
//!   ┌─ Vendor machine ─────────────────────────────────────────────────────┐
//!   │  keygen CLI holds Ed25519 PRIVATE key (never leaves this machine)    │
//!   │  Signs: SHA-256(canonical_payload_json)                              │
//!   │  Outputs: SUPERPOS-<b64url(payload)>.<b64url(signature)>             │
//!   └──────────────────────────────────────────────────────────────────────┘
//!                                │  license key string
//!                                ▼
//!   ┌─ Client machine ─────────────────────────────────────────────────────┐
//!   │  App holds Ed25519 PUBLIC key (32 bytes, obfuscated in binary)       │
//!   │  Verifies signature → reads payload → checks HWID + expiry           │
//!   │  Encrypts the verified blob with AES-256-GCM(HKDF(machine_uid))      │
//!   │  Writes encrypted file: superpos.lic                                 │
//!   └──────────────────────────────────────────────────────────────────────┘
//!
//! ## Why Ed25519 over AES-GCM for the license format?
//!   * AES-GCM requires a secret key embedded in the binary.  Anyone who
//!     reverse-engineers the binary can extract the key and forge licenses.
//!   * Ed25519 only requires the *public* key embedded.  The private key
//!     never leaves the vendor machine, so binary extraction yields nothing
//!     useful for an attacker.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::LicenseError;

// ─── Embedded public key ──────────────────────────────────────────────────────
//
// The 32-byte Ed25519 verifying key is stored XOR-obfuscated to make it
// slightly harder to locate and patch in a hex editor.
//
// HOW TO GENERATE YOUR PRODUCTION KEY PAIR:
//   Run the keygen CLI with the `--gen-keypair` flag:
//     cargo run --bin keygen -- --gen-keypair
//   This outputs:
//     PRIVATE_KEY_HEX = "..."  (store securely, never commit)
//     PUBLIC_KEY_XOR  = "..."  (paste here, commit to repo)
//
// The XOR mask (second constant) is arbitrary; change it in production.
//
// DEVELOPMENT PLACEHOLDER — replace before shipping:
const OBFUSCATED_PUBKEY: [u8; 32] = [
    0xa4, 0x99, 0x3c, 0x74, 0xe0, 0x4f, 0x8f, 0x46,
    0x6f, 0xd7, 0x0c, 0xea, 0x41, 0x59, 0x27, 0x54,
    0xaa, 0x8b, 0x54, 0xf1, 0x6f, 0xe1, 0xbf, 0x03,
    0x3a, 0xc6, 0xfd, 0x58, 0x8f, 0x99, 0x24, 0x93,
];

const XOR_MASK: [u8; 32] = [
    0xeb, 0x64, 0xc2, 0x4e, 0x84, 0x68, 0xb8, 0x18,
    0x78, 0xdf, 0x82, 0x78, 0x59, 0x31, 0x4e, 0x36,
    0xc3, 0x55, 0xe9, 0x53, 0x81, 0xec, 0xdd, 0x09,
    0x99, 0x9a, 0x5c, 0x0b, 0x00, 0x65, 0x3e, 0x05,
];

/// Deobfuscate and return the verifying key.
/// This is intentionally not `const` — we want it evaluated at runtime.
#[inline(never)]
pub fn verifying_key() -> Result<VerifyingKey, LicenseError> {
    let mut raw = [0u8; 32];
    for i in 0..32 {
        raw[i] = OBFUSCATED_PUBKEY[i] ^ XOR_MASK[i];
    }
    VerifyingKey::from_bytes(&raw).map_err(|_| LicenseError::Corrupt)
}

// ─── License wire format ──────────────────────────────────────────────────────
//
// A license key string looks like:
//   SUPERPOS-<base64url_no_pad(payload_json_bytes)>.<base64url_no_pad(sig_64_bytes)>
//
// The payload JSON is canonicalised (keys sorted) before signing so the
// signature is deterministic regardless of serialiser field order.

pub const LICENSE_PREFIX: &str = "SUPERPOS-";

/// Parse a raw license key string into (payload_bytes, signature).
/// Returns `LicenseError::Corrupt` for any format violation.
pub fn parse_license_key(key: &str) -> Result<(Vec<u8>, Signature), LicenseError> {
    let key = key.trim();

    // Strip prefix
    let body = key
        .strip_prefix(LICENSE_PREFIX)
        .ok_or(LicenseError::Corrupt)?;

    // Split at '.'
    let (payload_b64, sig_b64) = body.split_once('.').ok_or(LicenseError::Corrupt)?;

    let payload_bytes = B64.decode(payload_b64).map_err(|_| LicenseError::Corrupt)?;
    let sig_bytes     = B64.decode(sig_b64).map_err(|_| LicenseError::Corrupt)?;

    if sig_bytes.len() != 64 {
        return Err(LicenseError::Corrupt);
    }

    let sig_arr: [u8; 64] = sig_bytes.try_into().map_err(|_| LicenseError::Corrupt)?;
    let signature = Signature::from_bytes(&sig_arr);

    Ok((payload_bytes, signature))
}

/// Verify the Ed25519 signature over the payload bytes.
/// Returns the raw payload bytes if valid.
pub fn verify_signature(payload_bytes: &[u8], signature: &Signature) -> Result<(), LicenseError> {
    let vk = verifying_key()?;

    // We sign SHA-256(payload) to avoid the Ed25519 message-length limitation
    // and to prevent length-extension attacks.
    let digest = Sha256::digest(payload_bytes);

    vk.verify_strict(&digest, signature)
        .map_err(|_| LicenseError::SignatureInvalid)
}

// ─── Storage encryption ───────────────────────────────────────────────────────
//
// The license blob written to disk is:
//   AES-256-GCM(
//     key  = HKDF-SHA256(ikm=machine_uid_bytes, salt=STORAGE_SALT, info=STORAGE_INFO),
//     data = the raw license key string bytes,
//   )
//
// Prepend the 12-byte nonce so we can decrypt without storing it separately:
//   disk_bytes = nonce (12 bytes) || ciphertext (N bytes)
//
// Because the key is derived from machine_uid, copying the .lic file to
// another machine produces a decryption failure *before* Ed25519 is checked.

const STORAGE_SALT: &[u8] = b"superpos-lic-storage-v2-salt-2024";
const STORAGE_INFO: &[u8] = b"aes256gcm-license-file-key";

/// Derive the storage encryption key from the machine UID.
fn derive_storage_key(machine_uid: &str) -> Zeroizing<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(Some(STORAGE_SALT), machine_uid.as_bytes());
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(STORAGE_INFO, okm.as_mut())
        .expect("HKDF expand: 32-byte output is always valid");
    okm
}

/// Encrypt `plaintext` (the raw license key string) for storage on disk.
pub fn encrypt_for_storage(plaintext: &str, machine_uid: &str) -> Result<Vec<u8>, LicenseError> {
    let key_bytes = derive_storage_key(machine_uid);
    let key       = Key::<Aes256Gcm>::from_slice(key_bytes.as_ref());
    let cipher    = Aes256Gcm::new(key);

    // Generate a random 12-byte nonce
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| LicenseError::StorageError("encrypt failed".into()))?;

    // Prepend nonce
    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt a storage blob, returning the original license key string.
pub fn decrypt_from_storage(blob: &[u8], machine_uid: &str) -> Result<String, LicenseError> {
    if blob.len() < 13 {
        return Err(LicenseError::Corrupt);
    }

    let (nonce_bytes, ciphertext) = blob.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let key_bytes = derive_storage_key(machine_uid);
    let key       = Key::<Aes256Gcm>::from_slice(key_bytes.as_ref());
    let cipher    = Aes256Gcm::new(key);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| LicenseError::StorageError("decrypt failed — wrong machine?".into()))?;

    String::from_utf8(plaintext)
        .map_err(|_| LicenseError::Corrupt)
}

// ─── Signing helpers (used by keygen CLI only) ────────────────────────────────

#[cfg(feature = "keygen")]
pub mod signing {
    use super::*;
    use ed25519_dalek::{SigningKey};

    /// Sign the payload bytes. Used by the vendor keygen CLI.
    pub fn sign_payload(payload_bytes: &[u8], signing_key: &SigningKey) -> Signature {
        use ed25519_dalek::Signer;
        let digest = Sha256::digest(payload_bytes);
        signing_key.sign(&digest)
    }

    /// Build the full license key string from payload + signature.
    pub fn build_license_key(payload_bytes: &[u8], signature: &Signature) -> String {
        format!(
            "{}{}. {}",
            LICENSE_PREFIX,
            B64.encode(payload_bytes),
            B64.encode(signature.to_bytes())
        )
        // Replace space with empty (the format is PREFIX-PAYLOAD.SIG, no spaces)
        .replace(". ", ".")
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use machine_uid;

    #[test]
    fn storage_roundtrip() {
        let uid = machine_uid::get().unwrap_or("test-machine".into());
        let plaintext = "SUPERPOS-dGVzdA.c2lnbmF0dXJl";
        let encrypted = encrypt_for_storage(plaintext, &uid).unwrap();
        let decrypted = decrypt_from_storage(&encrypted, &uid).unwrap();
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn storage_fails_on_wrong_machine() {
        let uid1 = "machine-a";
        let uid2 = "machine-b";
        let plain = "SUPERPOS-test.sig";
        let enc = encrypt_for_storage(plain, uid1).unwrap();
        // Should fail when decrypting with a different machine's key
        assert!(decrypt_from_storage(&enc, uid2).is_err());
    }

    #[test]
    fn parse_bad_prefix_rejected() {
        assert!(parse_license_key("BADPREFIX-abc.def").is_err());
    }

    #[test]
    fn parse_missing_dot_rejected() {
        assert!(parse_license_key("SUPERPOS-abcdef").is_err());
    }
}