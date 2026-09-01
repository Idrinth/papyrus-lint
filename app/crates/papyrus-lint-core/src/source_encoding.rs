//! Reads `.psc` source files, which the Creation Kit and Papyrus compiler
//! both write out as Windows-1252 (CP1252) — Bethesda's default encoding
//! for the language — rather than UTF-8. Most scripts are pure ASCII, so
//! they round-trip fine as UTF-8, but any that contain CP1252-only bytes
//! (curly quotes, accented letters in comments/string literals, etc.)
//! aren't valid UTF-8 and previously made [`std::fs::read_to_string`] fail
//! outright, aborting an entire achlist run over a single such file.
//!
//! [`read_psc_source`] instead treats CP1252 as the fallback: valid UTF-8
//! is decoded as UTF-8 (so a script already saved as UTF-8, e.g. by an
//! editor, still reads correctly), and anything else is decoded as
//! CP1252, which — per the WHATWG Encoding Standard that `encoding_rs`
//! implements — maps every possible byte to some character, so this never
//! fails.

use std::io;
use std::path::Path;

/// Reads the file at `path`, decoding it as UTF-8 if it's valid UTF-8, or
/// as Windows-1252 (CP1252) otherwise. Only fails on the underlying I/O
/// error from reading the file itself.
pub fn read_psc_source(path: &Path) -> io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(decode_psc_source(&bytes))
}

/// Decodes `bytes` as UTF-8 if valid, or as Windows-1252 (CP1252)
/// otherwise, per [`read_psc_source`].
pub fn decode_psc_source(bytes: &[u8]) -> String {
    match String::from_utf8(bytes.to_vec()) {
        Ok(source) => source,
        Err(err) => {
            let (source, _encoding, _had_errors) = encoding_rs::WINDOWS_1252.decode(err.as_bytes());
            source.into_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_valid_utf8_as_utf8() {
        let bytes = "Function Foo() ; café".as_bytes();

        assert_eq!(decode_psc_source(bytes), "Function Foo() ; café");
    }

    #[test]
    fn decodes_empty_input() {
        assert_eq!(decode_psc_source(&[]), "");
    }

    #[test]
    fn preserves_a_utf8_byte_order_mark() {
        let bytes = b"\xEF\xBB\xBFScriptName Example";

        assert_eq!(decode_psc_source(bytes), "\u{FEFF}ScriptName Example");
    }

    #[test]
    fn falls_back_to_cp1252_for_non_utf8_bytes() {
        // 0x93/0x94 are CP1252's curly double quotes; neither is valid
        // UTF-8 on its own.
        let bytes = [b'"', 0x93, b'h', b'i', 0x94, b'"'];

        assert_eq!(decode_psc_source(&bytes), "\"\u{201C}hi\u{201D}\"");
    }

    #[test]
    fn falls_back_to_cp1252_for_accented_latin1_bytes() {
        // 0xE9 is CP1252's "é"; invalid as a standalone UTF-8 byte.
        let bytes = [b'c', b'a', b'f', 0xE9];

        assert_eq!(decode_psc_source(&bytes), "café");
    }

    #[test]
    fn reads_file_from_disk_with_fallback() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("Example.psc");
        std::fs::write(&path, [b'c', b'a', b'f', 0xE9]).expect("failed to write test file");

        let source = read_psc_source(&path).expect("reading should succeed");

        assert_eq!(source, "café");
    }

    #[test]
    fn propagates_io_errors() {
        let missing = Path::new("/nonexistent/path/does-not-exist.psc");

        assert!(read_psc_source(missing).is_err());
    }
}
