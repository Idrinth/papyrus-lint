//! Compatibility check between a cache entry's `linter_version` and the
//! running binary's own version.

/// The oldest linter release whose AST cache entries the running binary
/// still accepts. See the crate docs for when to bump this.
pub(crate) const MIN_COMPATIBLE_VERSION: &str = "1.46.0";

/// Parses a `major.minor.patch` version string into a comparable tuple.
/// Returns `None` for anything that doesn't parse that way, so a malformed
/// or unexpected version string is treated as incompatible rather than
/// panicking.
fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Version string stamped onto newly written cache entries.
///
/// Released binaries have `CARGO_PKG_VERSION` bumped to the linter's
/// release version (see `.github/workflows/release.yml`), which is at or
/// above [`MIN_COMPATIBLE_VERSION`]. Unreleased development builds still
/// carry the crates' placeholder `0.1.0`, which would otherwise fail the
/// compatibility check and make every `put`/`get` a miss. In that case we
/// stamp [`MIN_COMPATIBLE_VERSION`] instead, so a dev build can read back
/// the entries it just wrote, and a later bump of the floor still
/// invalidates them.
pub(crate) fn stamped_version() -> &'static str {
    match (
        parse_version(env!("CARGO_PKG_VERSION")),
        parse_version(MIN_COMPATIBLE_VERSION),
    ) {
        (Some(pkg), Some(min)) if pkg >= min => env!("CARGO_PKG_VERSION"),
        _ => MIN_COMPATIBLE_VERSION,
    }
}

/// Whether a cache entry written by `version` is still readable by this
/// binary, i.e. `version >= MIN_COMPATIBLE_VERSION`. Either version failing
/// to parse is treated as incompatible.
pub(crate) fn is_compatible_version(version: &str) -> bool {
    let Some(min) = parse_version(MIN_COMPATIBLE_VERSION) else {
        return false;
    };
    parse_version(version).is_some_and(|v| v >= min)
}

#[cfg(test)]
mod tests {
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
        assert!(is_compatible_version("1.46.1"));
        assert!(is_compatible_version("2.0.0"));
        assert!(!is_compatible_version("1.45.99"));
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
            parse_version(stamped_version()).unwrap()
                >= parse_version(MIN_COMPATIBLE_VERSION).unwrap()
        );
    }
}
