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

/// Which encoding a `.psc` file was actually read as (see
/// [`read_psc_source_with_encoding`]), so a later write-back of repaired
/// content can be encoded the same way it was read — otherwise fixing a
/// Windows-1252-encoded file would silently re-save it as UTF-8, changing
/// its encoding even though its content is (mostly) unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PscEncoding {
    Utf8,
    Windows1252,
}

/// Reads the file at `path`, decoding it as UTF-8 if it's valid UTF-8, or
/// as Windows-1252 (CP1252) otherwise. Only fails on the underlying I/O
/// error from reading the file itself.
pub fn read_psc_source(path: &Path) -> io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(decode_psc_source(&bytes))
}

/// Like [`read_psc_source`], but also returns the [`PscEncoding`] that was
/// used, for callers that may write repaired content back to `path` and
/// need to preserve its original on-disk encoding.
pub fn read_psc_source_with_encoding(path: &Path) -> io::Result<(String, PscEncoding)> {
    let bytes = std::fs::read(path)?;
    Ok(decode_psc_source_with_encoding(&bytes))
}

/// Decodes `bytes` as UTF-8 if valid, or as Windows-1252 (CP1252)
/// otherwise, per [`read_psc_source`].
pub fn decode_psc_source(bytes: &[u8]) -> String {
    decode_psc_source_with_encoding(bytes).0
}

/// Like [`decode_psc_source`], but also returns the [`PscEncoding`] that
/// was used to decode `bytes`.
pub fn decode_psc_source_with_encoding(bytes: &[u8]) -> (String, PscEncoding) {
    match String::from_utf8(bytes.to_vec()) {
        Ok(source) => (source, PscEncoding::Utf8),
        Err(err) => {
            let (source, _encoding, _had_errors) = encoding_rs::WINDOWS_1252.decode(err.as_bytes());
            (source.into_owned(), PscEncoding::Windows1252)
        }
    }
}

/// Encodes `source` as `encoding`'s bytes — the counterpart to
/// [`decode_psc_source_with_encoding`]/[`read_psc_source_with_encoding`].
/// A character with no Windows-1252 representation (never expected in
/// practice: every character an automatic fix can introduce is plain
/// ASCII, and everything read from a Windows-1252 file already round-trips
/// through it) falls back to a numeric character reference rather than
/// silently dropping data.
pub fn encode_psc_source(source: &str, encoding: PscEncoding) -> Vec<u8> {
    match encoding {
        PscEncoding::Utf8 => source.as_bytes().to_vec(),
        PscEncoding::Windows1252 => {
            let (bytes, _encoding, _had_unmappable_chars) =
                encoding_rs::WINDOWS_1252.encode(source);
            bytes.into_owned()
        }
    }
}

/// Writes `source` to `path`, encoded as `encoding` — the write-back
/// counterpart to [`read_psc_source_with_encoding`]. Used after applying
/// an automatic fix, so the file's on-disk encoding never changes even
/// though its content does.
pub fn write_psc_source(path: &Path, source: &str, encoding: PscEncoding) -> io::Result<()> {
    std::fs::write(path, encode_psc_source(source, encoding))
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
    fn cp1252_fallback_decodes_ascii_and_non_ascii_bytes_together() {
        // The invalid byte triggers the fallback for the complete input,
        // which must still preserve every ASCII byte around it.
        let bytes = b"ScriptName Example\r\n; price: \x8010\r\n";

        assert_eq!(
            decode_psc_source(bytes),
            "ScriptName Example\r\n; price: €10\r\n"
        );
    }

    #[test]
    fn cp1252_fallback_maps_every_input_byte() {
        // WHATWG Windows-1252 maps historically undefined bytes such as
        // 0x81 to their corresponding C1 control code rather than failing.
        assert_eq!(decode_psc_source(&[b'a', 0x81, b'b']), "a\u{81}b");
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
    fn reads_valid_utf8_file_from_disk_without_cp1252_reinterpretation() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("Example.psc");
        std::fs::write(&path, "ScriptName Example ; 漢字").expect("failed to write test file");

        let source = read_psc_source(&path).expect("reading should succeed");

        assert_eq!(source, "ScriptName Example ; 漢字");
    }

    #[test]
    fn propagates_io_errors() {
        let missing = Path::new("/nonexistent/path/does-not-exist.psc");

        assert!(read_psc_source(missing).is_err());
    }

    #[test]
    fn reports_windows_1252_for_non_utf8_bytes() {
        let (source, encoding) = decode_psc_source_with_encoding(&[b'c', b'a', b'f', 0xE9]);

        assert_eq!(source, "café");
        assert_eq!(encoding, PscEncoding::Windows1252);
    }

    #[test]
    fn reports_utf8_for_valid_utf8_bytes() {
        let (source, encoding) = decode_psc_source_with_encoding("café".as_bytes());

        assert_eq!(source, "café");
        assert_eq!(encoding, PscEncoding::Utf8);
    }

    #[test]
    fn round_trips_windows_1252_bytes_through_decode_and_encode() {
        let original = [b'c', b'a', b'f', 0xE9, b'\r', b'\n', 0x93, b'h', b'i', 0x94];

        let (source, encoding) = decode_psc_source_with_encoding(&original);
        let encoded = encode_psc_source(&source, encoding);

        assert_eq!(encoded, original);
    }

    #[test]
    fn encodes_utf8_source_as_utf8_bytes() {
        let bytes = encode_psc_source("ScriptName Example ; 漢字", PscEncoding::Utf8);

        assert_eq!(bytes, "ScriptName Example ; 漢字".as_bytes());
    }

    #[test]
    fn read_write_round_trip_preserves_a_windows_1252_file_byte_for_byte() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("Example.psc");
        let original = [b'c', b'a', b'f', 0xE9, b'\r', b'\n'];
        std::fs::write(&path, original).expect("failed to write test file");

        let (source, encoding) =
            read_psc_source_with_encoding(&path).expect("reading should succeed");
        write_psc_source(&path, &source, encoding).expect("writing should succeed");

        let bytes_on_disk = std::fs::read(&path).expect("failed to read back test file");
        assert_eq!(bytes_on_disk, original);
    }

    #[test]
    fn read_write_round_trip_preserves_a_utf8_file_byte_for_byte() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("Example.psc");
        let original = "ScriptName Example ; 漢字\r\n";
        std::fs::write(&path, original).expect("failed to write test file");

        let (source, encoding) =
            read_psc_source_with_encoding(&path).expect("reading should succeed");
        write_psc_source(&path, &source, encoding).expect("writing should succeed");

        let bytes_on_disk = std::fs::read(&path).expect("failed to read back test file");
        assert_eq!(bytes_on_disk, original.as_bytes());
    }
}
