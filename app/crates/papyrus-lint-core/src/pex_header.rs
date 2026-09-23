//! Strips the compiling machine's Windows username and computer name out
//! of a compiled `.pex` file's header.
//!
//! `PapyrusCompiler.exe` writes both directly into every script it
//! compiles, right after the source file name, as plain length-prefixed
//! strings in the header:
//!
//! ```text
//! uint32 magic
//! uint8  majorVersion
//! uint8  minorVersion
//! uint16 gameID
//! uint64 compilationTime
//! string sourceFileName   (uint16 length, then that many bytes)
//! string userName         (uint16 length, then that many bytes)
//! string machineName      (uint16 length, then that many bytes)
//! ```
//!
//! so a `.pex` shared as-is (e.g. bundled into a mod) leaks the machine it
//! was built on. Skyrim's compiler writes this header big-endian
//! (magic bytes `FA 57 C0 DE`); Fallout 4/Starfield's writes it
//! little-endian (`DE C0 57 FA`) — [`strip_personal_data`] detects which
//! from the magic and follows suit for the two-byte lengths it rewrites.

#[derive(Clone, Copy)]
enum Endianness {
    Big,
    Little,
}

const BE_MAGIC: [u8; 4] = [0xFA, 0x57, 0xC0, 0xDE];
const LE_MAGIC: [u8; 4] = [0xDE, 0xC0, 0x57, 0xFA];

/// The header fields preceding the three strings: magic (4) + majorVersion
/// (1) + minorVersion (1) + gameID (2) + compilationTime (8).
const STRINGS_START: usize = 4 + 1 + 1 + 2 + 8;

struct StringSpan {
    /// Offset of the string's 2-byte length prefix.
    start: usize,
    /// Offset just past the string's data.
    data_end: usize,
    len: u16,
}

fn read_u16(bytes: &[u8], pos: usize, endianness: Endianness) -> Option<u16> {
    let pair = bytes.get(pos..pos + 2)?;
    Some(match endianness {
        Endianness::Big => u16::from_be_bytes([pair[0], pair[1]]),
        Endianness::Little => u16::from_le_bytes([pair[0], pair[1]]),
    })
}

fn read_string_span(bytes: &[u8], pos: usize, endianness: Endianness) -> Option<StringSpan> {
    let len = read_u16(bytes, pos, endianness)?;
    let data_end = pos.checked_add(2)?.checked_add(len as usize)?;
    if data_end > bytes.len() {
        return None;
    }
    Some(StringSpan {
        start: pos,
        data_end,
        len,
    })
}

/// Returns `pex_bytes` with its `userName` and `machineName` header
/// strings replaced by empty strings, or `None` if either `pex_bytes`
/// doesn't look like a `.pex` file (unrecognized magic, or too short to
/// hold a full header) or both strings are already empty, meaning there's
/// nothing to strip.
pub fn strip_personal_data(pex_bytes: &[u8]) -> Option<Vec<u8>> {
    let magic: [u8; 4] = pex_bytes.get(0..4)?.try_into().ok()?;
    let endianness = if magic == BE_MAGIC {
        Endianness::Big
    } else if magic == LE_MAGIC {
        Endianness::Little
    } else {
        return None;
    };

    let source_file_name = read_string_span(pex_bytes, STRINGS_START, endianness)?;
    let user_name = read_string_span(pex_bytes, source_file_name.data_end, endianness)?;
    let machine_name = read_string_span(pex_bytes, user_name.data_end, endianness)?;

    if user_name.len == 0 && machine_name.len == 0 {
        return None;
    }

    let zero_len = match endianness {
        Endianness::Big => 0u16.to_be_bytes(),
        Endianness::Little => 0u16.to_le_bytes(),
    };

    let mut patched = Vec::with_capacity(pex_bytes.len());
    patched.extend_from_slice(&pex_bytes[..user_name.start]);
    patched.extend_from_slice(&zero_len);
    patched.extend_from_slice(&zero_len);
    patched.extend_from_slice(&pex_bytes[machine_name.data_end..]);
    Some(patched)
}

#[cfg(test)]
#[path = "pex_header_tests.rs"]
mod tests;
