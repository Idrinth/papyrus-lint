//! A small MD5 hashing helper for the "Export for AI" feature's redacted
//! source option: the desktop app's `hash_psc_file_md5` Tauri command
//! (`app/src-tauri/src/lib.rs`) and `papyrus-lint-cli`'s `--hash-source`
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
mod tests {
    use super::*;

    #[test]
    fn md5_hex_of_empty_string_matches_the_well_known_digest() {
        assert_eq!(md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn md5_hex_matches_the_same_computation_ast_cache_uses() {
        let source = "ScriptName Example\n";
        assert_eq!(
            md5_hex(source),
            format!("{:x}", md5::compute(source.as_bytes()))
        );
    }

    #[test]
    fn md5_hex_differs_for_different_content() {
        assert_ne!(md5_hex("a"), md5_hex("b"));
    }
}
