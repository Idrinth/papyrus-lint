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

#[test]
fn sha256_hex_of_empty_string_matches_the_well_known_digest() {
    assert_eq!(
        sha256_hex(""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn sha256_hex_differs_for_different_content() {
    assert_ne!(sha256_hex("a"), sha256_hex("b"));
}
