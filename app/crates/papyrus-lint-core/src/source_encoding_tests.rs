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
fn windows_1252_encoding_uses_numeric_references_for_unmappable_characters() {
    let bytes = encode_psc_source("; snowman: ☃", PscEncoding::Windows1252);

    assert_eq!(bytes, b"; snowman: &#9731;");
}

#[test]
fn windows_1252_encoding_preserves_representable_extended_characters() {
    let bytes = encode_psc_source("€ “quoted” café", PscEncoding::Windows1252);

    assert_eq!(
        bytes,
        [
            0x80, b' ', 0x93, b'q', b'u', b'o', b't', b'e', b'd', 0x94, b' ', b'c', b'a', b'f',
            0xE9
        ]
    );
}

#[test]
fn read_write_round_trip_preserves_a_windows_1252_file_byte_for_byte() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("Example.psc");
    let original = [b'c', b'a', b'f', 0xE9, b'\r', b'\n'];
    std::fs::write(&path, original).expect("failed to write test file");

    let (source, encoding) = read_psc_source_with_encoding(&path).expect("reading should succeed");
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

    let (source, encoding) = read_psc_source_with_encoding(&path).expect("reading should succeed");
    write_psc_source(&path, &source, encoding).expect("writing should succeed");

    let bytes_on_disk = std::fs::read(&path).expect("failed to read back test file");
    assert_eq!(bytes_on_disk, original.as_bytes());
}

#[test]
fn writing_a_repair_preserves_the_original_windows_1252_encoding() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, b"ScriptName Example\r\n; caf\xE9   \r\n")
        .expect("failed to write test file");

    let (source, encoding) = read_psc_source_with_encoding(&path).expect("reading should succeed");
    let repaired = source.replace("   \r\n", "\r\n");
    write_psc_source(&path, &repaired, encoding).expect("writing should succeed");

    assert_eq!(
        std::fs::read(&path).expect("failed to read back test file"),
        b"ScriptName Example\r\n; caf\xE9\r\n"
    );
}

#[test]
fn writing_a_repair_preserves_utf8_characters_without_reencoding_them() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n; 漢字   \n").expect("failed to write test file");

    let (source, encoding) = read_psc_source_with_encoding(&path).expect("reading should succeed");
    let repaired = source.replace("   \n", "\n");
    write_psc_source(&path, &repaired, encoding).expect("writing should succeed");

    assert_eq!(
        std::fs::read(&path).expect("failed to read back test file"),
        "ScriptName Example\n; 漢字\n".as_bytes()
    );
}

#[test]
fn read_with_encoding_propagates_io_errors() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing = dir.path().join("does-not-exist.psc");

    assert!(read_psc_source_with_encoding(&missing).is_err());
}

#[test]
fn write_propagates_io_errors() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing_parent = dir.path().join("missing-parent").join("Example.psc");

    assert!(write_psc_source(&missing_parent, "ScriptName Example", PscEncoding::Utf8).is_err());
}
