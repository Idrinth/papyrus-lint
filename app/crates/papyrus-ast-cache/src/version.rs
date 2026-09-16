//! Compatibility check between a cache entry's `linter_version` and the
//! running binary's own version.

/// The oldest linter release whose AST cache entries the running binary
/// still accepts. See the crate docs for when to bump this.
pub(crate) const MIN_COMPATIBLE_VERSION: &str = "1.36.0";

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
    fn is_compatible_version_accepts_the_minimum_and_anything_newer() {
        assert!(is_compatible_version(MIN_COMPATIBLE_VERSION));
        assert!(is_compatible_version("1.36.1"));
        assert!(is_compatible_version("2.0.0"));
        assert!(!is_compatible_version("1.35.99"));
        assert!(!is_compatible_version("not-a-version"));
    }
}
