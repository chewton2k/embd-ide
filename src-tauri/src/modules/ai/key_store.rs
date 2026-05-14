//! Encrypted key file store.
//!
//! Used as a fallback when the OS keyring is unavailable or returns errors.
//! Keys are encrypted with ChaCha20-Poly1305 (AEAD) using a 32-byte master
//! key. The master key itself is sourced (in this order) by the parent
//! module:
//!
//!   1. OS keyring entry `__file_key__` (preferred — never on disk)
//!   2. Newly-generated random key, stored in keyring
//!   3. Machine-derived key (last-resort obfuscation; documented limitation)
//!
//! File layout on disk:
//!
//!   ┌────────┬──────────────┬────────────────────────────┐
//!   │ 1 byte │   12 bytes   │  ciphertext + 16-byte tag  │
//!   │ ver=01 │     nonce    │  ChaCha20-Poly1305 sealed  │
//!   └────────┴──────────────┴────────────────────────────┘
//!
//! Each write generates a fresh random nonce. Writes are atomic: temp file
//! plus rename, with 0o600 permissions on Unix.
//!
//! Plaintext is JSON: `{ "<provider>": "<api-key>", ... }`.

use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use rand::RngCore;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const FILE_VERSION: u8 = 0x01;
pub const NONCE_SIZE: usize = 12;
pub const KEY_SIZE: usize = 32;
pub const TAG_SIZE: usize = 16;

/// Encode a plaintext blob to the on-disk versioned format.
///
/// Returns `Ok(blob)` on success. The returned bytes are safe to write to
/// disk; the caller is responsible for atomic placement and permissions.
pub fn encrypt(file_key: &[u8; KEY_SIZE], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(file_key));
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|e| format!("encrypt: {e}"))?;
    let mut out = Vec::with_capacity(1 + NONCE_SIZE + ciphertext.len());
    out.push(FILE_VERSION);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decode an on-disk blob back to plaintext.
///
/// Returns `Err` for any structural problem (truncated, wrong version,
/// failed AEAD verification). The caller treats `Err` as "no recoverable
/// data"; we never crash on a corrupted file.
pub fn decrypt(file_key: &[u8; KEY_SIZE], blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < 1 + NONCE_SIZE + TAG_SIZE {
        return Err("encrypted-keys file is too short".into());
    }
    if blob[0] != FILE_VERSION {
        return Err(format!(
            "unsupported encrypted-keys version: 0x{:02x}",
            blob[0]
        ));
    }
    let nonce_bytes = &blob[1..1 + NONCE_SIZE];
    let ciphertext = &blob[1 + NONCE_SIZE..];
    let cipher = ChaCha20Poly1305::new(Key::from_slice(file_key));
    cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| "encrypted-keys decryption failed (corrupted or wrong key)".to_string())
}

/// Filesystem location of the encrypted keys blob inside `base_dir`.
pub fn encrypted_path(base_dir: &Path) -> PathBuf {
    base_dir.join("keys.enc")
}

/// Read and decrypt the keys file. Returns an empty map if the file is
/// absent or unreadable. Returns `Err` only when the file exists but
/// cannot be decrypted (so the caller can surface a "corrupted" error
/// rather than silently losing keys).
pub fn read_all(
    base_dir: &Path,
    file_key: &[u8; KEY_SIZE],
) -> Result<HashMap<String, String>, String> {
    let path = encrypted_path(base_dir);
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("read keys.enc: {e}"))?;
    let plaintext = decrypt(file_key, &bytes)?;
    let map: HashMap<String, String> =
        serde_json::from_slice(&plaintext).map_err(|e| format!("parse keys.enc: {e}"))?;
    Ok(map)
}

/// Atomically write the encrypted keys map. Creates `base_dir` if needed,
/// applies 0o600 permissions on Unix, and uses a temp file + rename so a
/// crash mid-write cannot leave the file half-written.
pub fn write_all(
    base_dir: &Path,
    file_key: &[u8; KEY_SIZE],
    map: &HashMap<String, String>,
) -> Result<(), String> {
    std::fs::create_dir_all(base_dir).map_err(|e| format!("mkdir base: {e}"))?;
    let plaintext = serde_json::to_vec(map).map_err(|e| format!("serialize keys: {e}"))?;
    let blob = encrypt(file_key, &plaintext)?;
    let final_path = encrypted_path(base_dir);
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = base_dir.join(format!(".keys.enc.{pid}.{nanos}.tmp"));
    std::fs::write(&tmp, &blob).map_err(|e| format!("write tmp: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod tmp: {e}"))?;
    }
    std::fs::rename(&tmp, &final_path).map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

/// Remove the encrypted keys file if present. Errors are swallowed —
/// "delete a thing that doesn't exist" is success.
pub fn delete(base_dir: &Path) {
    let _ = std::fs::remove_file(encrypted_path(base_dir));
}

/// One-shot insert/update for a single provider. Reads, mutates, writes.
pub fn put(
    base_dir: &Path,
    file_key: &[u8; KEY_SIZE],
    provider: &str,
    key: &str,
) -> Result<(), String> {
    let mut map = read_all(base_dir, file_key).unwrap_or_default();
    map.insert(provider.to_string(), key.to_string());
    write_all(base_dir, file_key, &map)
}

/// One-shot remove for a single provider. Drops the file when the last
/// entry is removed so we don't leave an empty encrypted blob behind.
pub fn remove(
    base_dir: &Path,
    file_key: &[u8; KEY_SIZE],
    provider: &str,
) -> Result<(), String> {
    let mut map = match read_all(base_dir, file_key) {
        Ok(m) => m,
        // Corrupted / unreadable: remove the file outright so we recover
        // to a known-empty state.
        Err(_) => {
            delete(base_dir);
            return Ok(());
        }
    };
    if map.remove(provider).is_some() {
        if map.is_empty() {
            delete(base_dir);
            Ok(())
        } else {
            write_all(base_dir, file_key, &map)
        }
    } else {
        Ok(())
    }
}

/// Look up a single provider key. `Ok(None)` for missing entries.
pub fn get(
    base_dir: &Path,
    file_key: &[u8; KEY_SIZE],
    provider: &str,
) -> Result<Option<String>, String> {
    let map = read_all(base_dir, file_key)?;
    Ok(map.get(provider).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_bytes(seed: u8) -> [u8; KEY_SIZE] {
        let mut k = [0u8; KEY_SIZE];
        for (i, b) in k.iter_mut().enumerate() {
            *b = seed.wrapping_add(i as u8);
        }
        k
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let k = key_bytes(0x42);
        let pt = b"hello, secret world";
        let blob = encrypt(&k, pt).unwrap();
        // Header: 1 byte version + 12 byte nonce
        assert_eq!(blob[0], FILE_VERSION);
        assert!(blob.len() >= 1 + NONCE_SIZE + TAG_SIZE);
        let recovered = decrypt(&k, &blob).unwrap();
        assert_eq!(recovered, pt);
    }

    #[test]
    fn encrypt_uses_fresh_nonce_each_call() {
        let k = key_bytes(0x07);
        let pt = b"same plaintext";
        let a = encrypt(&k, pt).unwrap();
        let b = encrypt(&k, pt).unwrap();
        // With probability ~1 - 2^-96 the nonces (and thus ciphertexts) differ.
        assert_ne!(a, b, "two encrypt() calls must produce distinct blobs");
    }

    #[test]
    fn decrypt_rejects_truncated_blob() {
        let k = key_bytes(0x33);
        let blob = encrypt(&k, b"x").unwrap();
        let truncated = &blob[..blob.len() - 4];
        assert!(decrypt(&k, truncated).is_err());
    }

    #[test]
    fn decrypt_rejects_wrong_version() {
        let k = key_bytes(0x01);
        let mut blob = encrypt(&k, b"x").unwrap();
        blob[0] = 0xFF;
        assert!(decrypt(&k, &blob).is_err());
    }

    #[test]
    fn decrypt_rejects_wrong_key() {
        let blob = encrypt(&key_bytes(0xAA), b"x").unwrap();
        assert!(decrypt(&key_bytes(0xBB), &blob).is_err());
    }

    #[test]
    fn decrypt_rejects_tampered_ciphertext() {
        let k = key_bytes(0x99);
        let mut blob = encrypt(&k, b"important").unwrap();
        // Flip a bit in the ciphertext body.
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(decrypt(&k, &blob).is_err());
    }

    #[test]
    fn read_all_returns_empty_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let k = key_bytes(0x10);
        let map = read_all(dir.path(), &k).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn put_get_remove_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let k = key_bytes(0x55);

        put(dir.path(), &k, "openai", "sk-test").unwrap();
        put(dir.path(), &k, "anthropic", "ant-test").unwrap();

        assert_eq!(
            get(dir.path(), &k, "openai").unwrap(),
            Some("sk-test".to_string())
        );
        assert_eq!(
            get(dir.path(), &k, "anthropic").unwrap(),
            Some("ant-test".to_string())
        );
        assert_eq!(get(dir.path(), &k, "missing").unwrap(), None);

        remove(dir.path(), &k, "openai").unwrap();
        assert_eq!(get(dir.path(), &k, "openai").unwrap(), None);
        assert_eq!(
            get(dir.path(), &k, "anthropic").unwrap(),
            Some("ant-test".to_string())
        );
    }

    #[test]
    fn remove_last_entry_deletes_file() {
        let dir = tempfile::tempdir().unwrap();
        let k = key_bytes(0xC3);
        put(dir.path(), &k, "openai", "sk-test").unwrap();
        assert!(encrypted_path(dir.path()).exists());

        remove(dir.path(), &k, "openai").unwrap();
        assert!(!encrypted_path(dir.path()).exists());
    }

    #[test]
    fn read_all_propagates_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let k = key_bytes(0x77);
        // Write garbage where the encrypted blob should be.
        std::fs::write(encrypted_path(dir.path()), b"\xFF\xFF\xFF").unwrap();
        let err = read_all(dir.path(), &k).unwrap_err();
        // Just sanity: caller gets an error instead of crashing or
        // silently wiping the file.
        assert!(!err.is_empty());
    }

    #[test]
    fn write_all_creates_dir_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("does/not/exist/yet");
        let k = key_bytes(0x1A);
        let mut map = HashMap::new();
        map.insert("x".into(), "y".into());
        write_all(&nested, &k, &map).unwrap();
        assert!(nested.exists());
        assert!(encrypted_path(&nested).exists());
    }

    #[cfg(unix)]
    #[test]
    fn write_all_sets_unix_permissions_to_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let k = key_bytes(0x2B);
        let mut map = HashMap::new();
        map.insert("x".into(), "y".into());
        write_all(dir.path(), &k, &map).unwrap();
        let meta = std::fs::metadata(encrypted_path(dir.path())).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600 perms, got {mode:o}");
    }
}
