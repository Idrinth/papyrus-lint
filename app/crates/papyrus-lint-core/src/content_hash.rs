//! Hashing helpers for source text: MD5 for the "Export for AI" redacted
//! source option, and SHA-256 for the on-disk script-collision cache used
//! by `conflicting-script-versions`.
//!
//! The desktop app's `hash_psc_file_md5` Tauri command
//! (`app/src-tauri/src/files.rs`) and `papyrus-lint-cli`'s `--hash-source`
//! flag both report a script's MD5 digest instead of its full text, so an
//! assistant can still tell files apart, or notice a file changed between
//! exports, without seeing its actual code. Computed the same way
//! [`crate::ast_cache`] already hashes a script's content for its own
//! change-detection cache key, so the same digest identifies the same
//! source everywhere it's used.

use sha2::{Digest, Sha256};

/// The lowercase hex MD5 digest of `content`.
pub fn md5_hex(content: &str) -> String {
    format!("{:x}", md5::compute(content.as_bytes()))
}

/// The lowercase hex SHA-256 digest of `content`.
pub fn sha256_hex(content: &str) -> String {
    sha256_bytes(content.as_bytes())
}

pub(crate) fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "content_hash_tests.rs"]
mod tests;
