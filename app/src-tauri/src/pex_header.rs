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
mod tests {
    use super::*;

    fn push_be_string(bytes: &mut Vec<u8>, s: &str) {
        bytes.extend_from_slice(&(s.len() as u16).to_be_bytes());
        bytes.extend_from_slice(s.as_bytes());
    }

    fn push_le_string(bytes: &mut Vec<u8>, s: &str) {
        bytes.extend_from_slice(&(s.len() as u16).to_le_bytes());
        bytes.extend_from_slice(s.as_bytes());
    }

    fn sample_be_header(source: &str, user: &str, machine: &str, trailer: &[u8]) -> Vec<u8> {
        let mut bytes = BE_MAGIC.to_vec();
        bytes.push(3); // majorVersion
        bytes.push(9); // minorVersion
        bytes.extend_from_slice(&1u16.to_be_bytes()); // gameID
        bytes.extend_from_slice(&0u64.to_be_bytes()); // compilationTime
        push_be_string(&mut bytes, source);
        push_be_string(&mut bytes, user);
        push_be_string(&mut bytes, machine);
        bytes.extend_from_slice(trailer);
        bytes
    }

    #[test]
    fn strips_username_and_machine_name_from_a_big_endian_header() {
        let bytes = sample_be_header("Foo.psc", "SomeUser", "SOME-PC", &[0xAA, 0xBB, 0xCC]);

        let patched = strip_personal_data(&bytes).expect("should strip");

        let expected = sample_be_header("Foo.psc", "", "", &[0xAA, 0xBB, 0xCC]);
        assert_eq!(patched, expected);
    }

    #[test]
    fn strips_a_little_endian_header_using_little_endian_lengths() {
        let mut bytes = LE_MAGIC.to_vec();
        bytes.push(1);
        bytes.push(0);
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        push_le_string(&mut bytes, "Foo.psc");
        push_le_string(&mut bytes, "SomeUser");
        push_le_string(&mut bytes, "SOME-PC");
        bytes.extend_from_slice(&[0x01, 0x02]);

        let patched = strip_personal_data(&bytes).expect("should strip");

        let mut expected = LE_MAGIC.to_vec();
        expected.push(1);
        expected.push(0);
        expected.extend_from_slice(&2u16.to_le_bytes());
        expected.extend_from_slice(&0u64.to_le_bytes());
        push_le_string(&mut expected, "Foo.psc");
        push_le_string(&mut expected, "");
        push_le_string(&mut expected, "");
        expected.extend_from_slice(&[0x01, 0x02]);
        assert_eq!(patched, expected);
    }

    #[test]
    fn returns_none_when_both_strings_are_already_empty() {
        let bytes = sample_be_header("Foo.psc", "", "", &[]);

        assert!(strip_personal_data(&bytes).is_none());
    }

    #[test]
    fn strips_a_lone_non_empty_username_leaving_an_empty_machine_name() {
        let bytes = sample_be_header("Foo.psc", "SomeUser", "", &[]);

        let patched = strip_personal_data(&bytes).expect("should strip");

        assert_eq!(patched, sample_be_header("Foo.psc", "", "", &[]));
    }

    #[test]
    fn strips_a_lone_non_empty_machine_name_leaving_an_empty_username() {
        let bytes = sample_be_header("Foo.psc", "", "SOME-PC", &[]);

        let patched = strip_personal_data(&bytes).expect("should strip");

        assert_eq!(patched, sample_be_header("Foo.psc", "", "", &[]));
    }

    #[test]
    fn preserves_all_bytes_after_the_header() {
        let trailer = [0x00, 0xFA, 0x57, 0xC0, 0xDE, 0xFF];
        let bytes = sample_be_header("Foo.psc", "SomeUser", "SOME-PC", &trailer);

        let patched = strip_personal_data(&bytes).expect("should strip");

        assert!(patched.ends_with(&trailer));
        assert_eq!(patched, sample_be_header("Foo.psc", "", "", &trailer));
    }

    #[test]
    fn returns_none_for_unrecognized_magic() {
        let bytes = vec![0, 1, 2, 3, 4, 5, 6, 7];

        assert!(strip_personal_data(&bytes).is_none());
    }

    #[test]
    fn returns_none_for_a_truncated_header() {
        let mut bytes = BE_MAGIC.to_vec();
        bytes.extend_from_slice(&[0, 0]);

        assert!(strip_personal_data(&bytes).is_none());
    }

    #[test]
    fn returns_none_when_even_the_magic_is_truncated() {
        assert!(strip_personal_data(&BE_MAGIC[..BE_MAGIC.len() - 1]).is_none());
    }

    #[test]
    fn returns_none_when_the_username_length_is_missing() {
        let mut bytes = BE_MAGIC.to_vec();
        bytes.extend_from_slice(&[0; STRINGS_START - BE_MAGIC.len()]);
        push_be_string(&mut bytes, "Foo.psc");

        assert!(strip_personal_data(&bytes).is_none());
    }

    #[test]
    fn returns_none_when_the_machine_name_length_is_missing() {
        let mut bytes = BE_MAGIC.to_vec();
        bytes.extend_from_slice(&[0; STRINGS_START - BE_MAGIC.len()]);
        push_be_string(&mut bytes, "Foo.psc");
        push_be_string(&mut bytes, "SomeUser");

        assert!(strip_personal_data(&bytes).is_none());
    }

    #[test]
    fn returns_none_when_the_source_file_name_overruns_the_buffer() {
        let mut bytes = BE_MAGIC.to_vec();
        bytes.extend_from_slice(&[0; STRINGS_START - BE_MAGIC.len()]);
        bytes.extend_from_slice(&0xFFFFu16.to_be_bytes());

        assert!(strip_personal_data(&bytes).is_none());
    }

    #[test]
    fn returns_none_when_a_string_length_overruns_the_buffer() {
        let mut bytes = sample_be_header("Foo.psc", "", "", &[]);
        let len = bytes.len();
        // Claim a userName length far longer than the remaining bytes.
        bytes[len - 4..len - 2].copy_from_slice(&0xFFFFu16.to_be_bytes());

        assert!(strip_personal_data(&bytes).is_none());
    }

    #[test]
    fn returns_none_when_the_machine_name_length_overruns_the_buffer() {
        let mut bytes = sample_be_header("Foo.psc", "SomeUser", "SOME-PC", &[]);
        let machine_length_offset = bytes.len() - "SOME-PC".len() - 2;
        bytes[machine_length_offset..machine_length_offset + 2]
            .copy_from_slice(&0xFFFFu16.to_be_bytes());

        assert!(strip_personal_data(&bytes).is_none());
    }
}
