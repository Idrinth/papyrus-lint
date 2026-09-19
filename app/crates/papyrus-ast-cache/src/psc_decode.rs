//! Decode `.psc` bytes the same way `papyrus_lint_core::source_encoding`
//! does: UTF-8 when valid, otherwise Windows-1252. Kept as a standalone
//! file so `build.rs` can `#[path]`-include it without pulling the rest
//! of this crate in. Not compiled into the library itself.

/// Decodes `bytes` as UTF-8 if valid, or as Windows-1252 otherwise.
pub fn decode_psc_bytes(bytes: &[u8]) -> String {
    match String::from_utf8(bytes.to_vec()) {
        Ok(source) => source,
        Err(err) => encoding_rs::WINDOWS_1252
            .decode(err.as_bytes())
            .0
            .into_owned(),
    }
}
