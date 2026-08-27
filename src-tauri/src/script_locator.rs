//! Locates Papyrus `.psc` source files by case-insensitive name.

use std::fs;
use std::path::{Path, PathBuf};

/// Directories, relative to a project root, conventionally used to store
/// Papyrus script sources. Also used by [`crate::compiler`] to build the
/// compiler's `-i` argument.
pub(crate) const CANDIDATE_DIRS: [&str; 2] = ["scripts/source", "source/scripts"];

/// Searches `root/scripts/source` and `root/source/scripts` for a `.psc`
/// file matching `name`, case-insensitively. `name` may be given with or
/// without the `.psc` extension.
///
/// Returns the path to the first match found, or `None` if neither
/// location contains a matching file.
pub fn find_psc_file(root: &Path, name: &str) -> Option<PathBuf> {
    let name_lower = name.to_ascii_lowercase();
    let target = if name_lower.ends_with(".psc") {
        name_lower
    } else {
        format!("{name_lower}.psc")
    };

    for dir in CANDIDATE_DIRS {
        let Ok(entries) = fs::read_dir(root.join(dir)) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let matches = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|file_name| file_name.to_ascii_lowercase() == target);

            if matches && path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, "").expect("failed to write test script file");
        path
    }

    #[test]
    fn finds_exact_match_in_scripts_source() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = root.path().join("scripts/source");
        fs::create_dir_all(&source_dir).expect("failed to create source dir");
        let expected = write_file(&source_dir, "Foo.psc");

        let result = find_psc_file(root.path(), "Foo.psc");

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn finds_case_insensitive_match_in_source_scripts() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = root.path().join("source/scripts");
        fs::create_dir_all(&source_dir).expect("failed to create source dir");
        let expected = write_file(&source_dir, "Foo.psc");

        let result = find_psc_file(root.path(), "fOO.PSC");

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn appends_extension_when_omitted() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = root.path().join("scripts/source");
        fs::create_dir_all(&source_dir).expect("failed to create source dir");
        let expected = write_file(&source_dir, "Foo.psc");

        let result = find_psc_file(root.path(), "foo");

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn prefers_scripts_source_over_source_scripts() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let scripts_source = root.path().join("scripts/source");
        let source_scripts = root.path().join("source/scripts");
        fs::create_dir_all(&scripts_source).expect("failed to create scripts/source dir");
        fs::create_dir_all(&source_scripts).expect("failed to create source/scripts dir");
        let expected = write_file(&scripts_source, "Foo.psc");
        write_file(&source_scripts, "Foo.psc");

        let result = find_psc_file(root.path(), "Foo.psc");

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn returns_none_when_no_match_found() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = root.path().join("scripts/source");
        fs::create_dir_all(&source_dir).expect("failed to create source dir");
        write_file(&source_dir, "Bar.psc");

        let result = find_psc_file(root.path(), "Foo.psc");

        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_when_neither_directory_exists() {
        let root = tempfile::tempdir().expect("failed to create temp dir");

        let result = find_psc_file(root.path(), "Foo.psc");

        assert_eq!(result, None);
    }
}
