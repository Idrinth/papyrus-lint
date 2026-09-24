use super::*;

#[test]
fn parse_version_rejects_malformed_strings() {
    assert_eq!(parse_version("1.11.0"), Some((1, 11, 0)));
    assert_eq!(parse_version("1.11"), None);
    assert_eq!(parse_version("1.11.x"), None);
    assert_eq!(parse_version(""), None);
}

#[test]
fn parse_version_requires_exactly_three_numeric_components() {
    assert_eq!(parse_version("1.13.0.1"), None);
    assert_eq!(parse_version("1.13.0-beta"), None);
    assert_eq!(parse_version("v1.13.0"), None);
    assert_eq!(parse_version("18446744073709551616.0.0"), None);
}

#[test]
fn parse_version_rejects_whitespace_and_empty_components() {
    assert_eq!(parse_version(" 1.36.0"), None);
    assert_eq!(parse_version("1.36.0 "), None);
    assert_eq!(parse_version("1.36.0\n"), None);
    assert_eq!(parse_version("1..0"), None);
    assert_eq!(parse_version("1.36."), None);
    assert_eq!(parse_version(".36.0"), None);
    assert_eq!(parse_version("1.36.0+build"), None);
    assert_eq!(parse_version("-1.36.0"), None);
}

#[test]
fn parse_version_accepts_leading_zeros_as_the_numeric_value() {
    assert_eq!(parse_version("01.36.00"), Some((1, 36, 0)));
    assert_eq!(parse_version("1.036.0"), Some((1, 36, 0)));
    // `u64::from_str` accepts a leading `+`.
    assert_eq!(parse_version("+1.36.0"), Some((1, 36, 0)));
}

#[test]
fn is_compatible_version_accepts_the_minimum_and_anything_newer() {
    assert!(is_compatible_version(MIN_COMPATIBLE_VERSION));
    assert!(is_compatible_version("1.48.1"));
    assert!(is_compatible_version("2.0.0"));
    assert!(!is_compatible_version("1.47.99"));
    assert!(!is_compatible_version("not-a-version"));
}

#[test]
fn is_compatible_version_rejects_an_older_major() {
    assert!(!is_compatible_version("0.99.99"));
    assert!(!is_compatible_version("0.1.0"));
    assert!(!is_compatible_version("0.0.0"));
}

#[test]
fn stamped_version_is_always_compatible() {
    assert!(is_compatible_version(stamped_version()));
    assert!(
        parse_version(stamped_version()).unwrap() >= parse_version(MIN_COMPATIBLE_VERSION).unwrap()
    );
}
