//! Parser for `.achlist` files: JSON arrays of file paths, each resolved
//! relative to the directory containing the achlist file itself.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum AchlistError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for AchlistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AchlistError::Io(err) => write!(f, "failed to read achlist file: {err}"),
            AchlistError::Json(err) => write!(f, "failed to parse achlist file: {err}"),
        }
    }
}

impl std::error::Error for AchlistError {}

impl From<std::io::Error> for AchlistError {
    fn from(err: std::io::Error) -> Self {
        AchlistError::Io(err)
    }
}

impl From<serde_json::Error> for AchlistError {
    fn from(err: serde_json::Error) -> Self {
        AchlistError::Json(err)
    }
}

/// Parses an `.achlist` file into the paths of the files it lists.
///
/// Each entry in the JSON array is resolved relative to the directory
/// containing `achlist_path`, since achlist entries are only meaningful
/// relative to the file they're listed in. Entries are conventionally
/// listed relative to the game's install directory (e.g.
/// `Data\SCRIPTS\SOURCE\Foo.psc`), so when `achlist_path` itself lives
/// directly inside a `Data` folder, a leading `Data` path component on an
/// entry is redundant and is stripped before joining, to avoid resolving
/// to a nonexistent `Data\Data\...` path.
pub fn parse_achlist(achlist_path: &Path) -> Result<Vec<PathBuf>, AchlistError> {
    let contents = fs::read_to_string(achlist_path)?;
    let entries: Vec<String> = serde_json::from_str(&contents)?;

    let base_dir = achlist_path.parent().unwrap_or_else(|| Path::new(""));

    Ok(entries
        .into_iter()
        .map(|entry| base_dir.join(strip_redundant_data_prefix(base_dir, &entry)))
        .collect())
}

/// If `base_dir` is itself a `Data` folder and `entry` starts with a
/// redundant `Data` path component, strips that component (and its
/// following separator) so joining `entry` onto `base_dir` doesn't
/// produce a doubled `Data\Data\...` path. Returns `entry` unchanged
/// otherwise.
fn strip_redundant_data_prefix(base_dir: &Path, entry: &str) -> String {
    let base_is_data = base_dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("data"));

    if !base_is_data {
        return entry.to_string();
    }

    let mut parts = entry.splitn(2, ['/', '\\']);
    let first = parts.next().unwrap_or("");

    match parts.next() {
        Some(rest) if first.eq_ignore_ascii_case("data") => rest.to_string(),
        _ => entry.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_achlist(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, contents).expect("failed to write test achlist file");
        path
    }

    #[test]
    fn parses_relative_paths_against_achlist_directory() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let achlist_path = write_achlist(
            dir.path(),
            "sources.achlist",
            r#"["scripts/Foo.psc", "../shared/Bar.psc"]"#,
        );

        let result = parse_achlist(&achlist_path).expect("parsing should succeed");

        assert_eq!(
            result,
            vec![
                dir.path().join("scripts/Foo.psc"),
                dir.path().join("../shared/Bar.psc"),
            ]
        );
    }

    #[test]
    fn strips_redundant_data_prefix_when_achlist_lives_in_data_dir() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let data_dir = root.path().join("Data");
        fs::create_dir_all(&data_dir).expect("failed to create Data dir");
        let achlist_path = write_achlist(
            &data_dir,
            "sources.achlist",
            r#"["Data\\SCRIPTS\\SOURCE\\Foo.psc"]"#,
        );

        let result = parse_achlist(&achlist_path).expect("parsing should succeed");

        assert_eq!(result, vec![data_dir.join("SCRIPTS\\SOURCE\\Foo.psc")]);
    }

    #[test]
    fn strips_redundant_data_prefix_case_insensitively() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let data_dir = root.path().join("data");
        fs::create_dir_all(&data_dir).expect("failed to create data dir");
        let achlist_path = write_achlist(&data_dir, "sources.achlist", r#"["DATA/Foo.psc"]"#);

        let result = parse_achlist(&achlist_path).expect("parsing should succeed");

        assert_eq!(result, vec![data_dir.join("Foo.psc")]);
    }

    #[test]
    fn leaves_entry_unchanged_when_achlist_directory_is_not_named_data() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let achlist_path = write_achlist(dir.path(), "sources.achlist", r#"["Data/Foo.psc"]"#);

        let result = parse_achlist(&achlist_path).expect("parsing should succeed");

        assert_eq!(result, vec![dir.path().join("Data/Foo.psc")]);
    }

    #[test]
    fn leaves_entry_unchanged_when_it_has_no_data_prefix() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let data_dir = root.path().join("Data");
        fs::create_dir_all(&data_dir).expect("failed to create Data dir");
        let achlist_path = write_achlist(
            &data_dir,
            "sources.achlist",
            r#"["SCRIPTS\\SOURCE\\Foo.psc"]"#,
        );

        let result = parse_achlist(&achlist_path).expect("parsing should succeed");

        assert_eq!(result, vec![data_dir.join("SCRIPTS\\SOURCE\\Foo.psc")]);
    }

    #[test]
    fn parses_empty_list() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let achlist_path = write_achlist(dir.path(), "empty.achlist", "[]");

        let result = parse_achlist(&achlist_path).expect("parsing should succeed");

        assert!(result.is_empty());
    }

    #[test]
    fn errors_on_missing_file() {
        let missing_path = PathBuf::from("/nonexistent/path/does-not-exist.achlist");

        let result = parse_achlist(&missing_path);

        assert!(matches!(result, Err(AchlistError::Io(_))));
    }

    #[test]
    fn errors_on_invalid_json() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let achlist_path = write_achlist(dir.path(), "broken.achlist", "not valid json");

        let result = parse_achlist(&achlist_path);

        assert!(matches!(result, Err(AchlistError::Json(_))));
    }

    #[test]
    fn errors_on_non_array_json() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let achlist_path = write_achlist(dir.path(), "object.achlist", r#"{"not": "a list"}"#);

        let result = parse_achlist(&achlist_path);

        assert!(matches!(result, Err(AchlistError::Json(_))));
    }
}
