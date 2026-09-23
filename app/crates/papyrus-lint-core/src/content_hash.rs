//! A small MD5 hashing helper for the "Export for AI" feature's redacted
//! source option: the desktop app's `hash_psc_file_md5` Tauri command
//! (`app/src-tauri/src/files.rs`) and `papyrus-lint-cli`'s `--hash-source`
//! flag both report a script's MD5 digest instead of its full text, so an
//! assistant can still tell files apart, or notice a file changed between
//! exports, without seeing its actual code. Computed the same way
//! [`crate::ast_cache`] already hashes a script's content for its own
//! change-detection cache key, so the same digest identifies the same
//! source everywhere it's used.

/// The lowercase hex MD5 digest of `content`.
pub fn md5_hex(content: &str) -> String {
    format!("{:x}", md5::compute(content.as_bytes()))
}

#[cfg(test)]
#[path = "content_hash_tests.rs"]
mod tests;
