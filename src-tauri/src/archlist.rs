//! Parser for `.archlist` files: JSON arrays of file paths, each resolved
//! relative to the directory containing the archlist file itself.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ArchlistError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for ArchlistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchlistError::Io(err) => write!(f, "failed to read archlist file: {err}"),
            ArchlistError::Json(err) => write!(f, "failed to parse archlist file: {err}"),
        }
    }
}

impl std::error::Error for ArchlistError {}

impl From<std::io::Error> for ArchlistError {
    fn from(err: std::io::Error) -> Self {
        ArchlistError::Io(err)
    }
}

impl From<serde_json::Error> for ArchlistError {
    fn from(err: serde_json::Error) -> Self {
        ArchlistError::Json(err)
    }
}

/// Parses an `.archlist` file into the paths of the files it lists.
///
/// Each entry in the JSON array is resolved relative to the directory
/// containing `archlist_path`, since archlist entries are only meaningful
/// relative to the file they're listed in.
pub fn parse_archlist(archlist_path: &Path) -> Result<Vec<PathBuf>, ArchlistError> {
    let contents = fs::read_to_string(archlist_path)?;
    let entries: Vec<String> = serde_json::from_str(&contents)?;

    let base_dir = archlist_path.parent().unwrap_or_else(|| Path::new(""));

    Ok(entries
        .into_iter()
        .map(|entry| base_dir.join(entry))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_archlist(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, contents).expect("failed to write test archlist file");
        path
    }

    #[test]
    fn parses_relative_paths_against_archlist_directory() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let archlist_path = write_archlist(
            dir.path(),
            "sources.archlist",
            r#"["scripts/Foo.psc", "../shared/Bar.psc"]"#,
        );

        let result = parse_archlist(&archlist_path).expect("parsing should succeed");

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
        let archlist_path = write_archlist(dir.path(), "empty.archlist", "[]");

        let result = parse_archlist(&archlist_path).expect("parsing should succeed");

        assert!(result.is_empty());
    }

    #[test]
    fn errors_on_missing_file() {
        let missing_path = PathBuf::from("/nonexistent/path/does-not-exist.archlist");

        let result = parse_archlist(&missing_path);

        assert!(matches!(result, Err(ArchlistError::Io(_))));
    }

    #[test]
    fn errors_on_invalid_json() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let archlist_path = write_archlist(dir.path(), "broken.archlist", "not valid json");

        let result = parse_archlist(&archlist_path);

        assert!(matches!(result, Err(ArchlistError::Json(_))));
    }

    #[test]
    fn errors_on_non_array_json() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let archlist_path = write_archlist(dir.path(), "object.archlist", r#"{"not": "a list"}"#);

        let result = parse_archlist(&archlist_path);

        assert!(matches!(result, Err(ArchlistError::Json(_))));
    }
}
