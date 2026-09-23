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
#[path = "achlist_tests.rs"]
mod tests;
