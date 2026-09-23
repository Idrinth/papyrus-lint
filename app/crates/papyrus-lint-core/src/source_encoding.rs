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
#[path = "source_encoding_tests.rs"]
mod tests;
