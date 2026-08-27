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
/// relative to the file they're listed in.
pub fn parse_achlist(achlist_path: &Path) -> Result<Vec<PathBuf>, AchlistError> {
    let contents = fs::read_to_string(achlist_path)?;
    let entries: Vec<String> = serde_json::from_str(&contents)?;

    let base_dir = achlist_path.parent().unwrap_or_else(|| Path::new(""));

    Ok(entries
        .into_iter()
        .map(|entry| base_dir.join(entry))
        .collect())
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
