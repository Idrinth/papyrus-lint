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

#[cfg(test)]
mod tests {
    use super::decode_psc_bytes;

    #[test]
    fn valid_utf8_is_preserved() {
        let source = "ScriptName Café\n; λ\n";

        assert_eq!(decode_psc_bytes(source.as_bytes()), source);
    }

    #[test]
    fn invalid_utf8_is_decoded_as_windows_1252() {
        let source = b"ScriptName Price\n; \x80 and \x93quotes\x94\n";

        assert_eq!(
            decode_psc_bytes(source),
            "ScriptName Price\n; € and “quotes”\n"
        );
    }
}
