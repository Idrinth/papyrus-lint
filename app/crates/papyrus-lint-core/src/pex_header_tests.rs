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
fn preserves_the_source_name_and_fixed_header_bytes_exactly() {
    let bytes = sample_be_header("Nested/Name (final).psc", "user", "machine", &[]);

    let patched = strip_personal_data(&bytes).expect("should strip");
    let expected = sample_be_header("Nested/Name (final).psc", "", "", &[]);

    assert_eq!(patched, expected);
    assert_eq!(&patched[..STRINGS_START], &bytes[..STRINGS_START]);
}

#[test]
fn strips_non_utf8_personal_data_as_opaque_header_bytes() {
    let mut bytes = sample_be_header("Foo.psc", "", "", &[0xCA, 0xFE]);
    let user_length_offset = STRINGS_START + 2 + "Foo.psc".len();
    bytes.splice(
        user_length_offset..user_length_offset + 4,
        [0, 2, 0xFF, 0x80, 0, 1, 0xFE],
    );

    let patched = strip_personal_data(&bytes).expect("binary names should still be stripped");

    assert_eq!(patched, sample_be_header("Foo.psc", "", "", &[0xCA, 0xFE]));
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
