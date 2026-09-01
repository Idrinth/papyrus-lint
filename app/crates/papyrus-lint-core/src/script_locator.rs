//! Locates Papyrus `.psc` source files by case-insensitive name.

use std::fs;
use std::path::{Path, PathBuf};

/// Directories, relative to a project root, conventionally used to store
/// Papyrus script sources. Also used by the desktop app's `compiler`
/// module to build the compiler's `-i` argument.
pub const CANDIDATE_DIRS: [&str; 2] = ["scripts/source", "source/scripts"];

/// Searches `root/scripts/source`, `root/source/scripts`, and then each of
/// `additional_roots` (in order) for a `.psc` file matching `name`,
/// case-insensitively. `name` may be given with or without the `.psc`
/// extension. Each entry in `additional_roots` is resolved relative to
/// `root` unless it's already absolute (see [`resolve_additional_roots`]).
///
/// Returns the path to the first match found, or `None` if none of those
/// locations contains a matching file.
pub fn find_psc_file(root: &Path, name: &str, additional_roots: &[String]) -> Option<PathBuf> {
    let name_lower = name.to_ascii_lowercase();
    let target = if name_lower.ends_with(".psc") {
        name_lower
    } else {
        format!("{name_lower}.psc")
    };

    let dirs = CANDIDATE_DIRS
        .iter()
        .map(|dir| root.join(dir))
        .chain(resolve_additional_roots(root, additional_roots));

    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
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

/// Resolves each of `roots` against `root`: an absolute entry is used as-is,
/// a relative one is joined onto `root`. A root ending in `source/scripts`
/// or `scripts/source` is followed by its counterpart beneath the same
/// parent, so projects containing both layouts need only configure or infer
/// one of them. Used to turn a project's
/// user-configured `additional_script_roots` (see
/// [`crate::config::load_script_roots`]) into directories to search
/// alongside [`CANDIDATE_DIRS`].
pub fn resolve_additional_roots(root: &Path, roots: &[String]) -> Vec<PathBuf> {
    let mut resolved = Vec::new();

    for entry in roots {
        let path = {
            let path = Path::new(entry);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                root.join(path)
            }
        };

        if !resolved.contains(&path) {
            resolved.push(path.clone());
        }

        let components = path
            .file_name()
            .and_then(|component| component.to_str())
            .zip(
                path.parent()
                    .and_then(Path::file_name)
                    .and_then(|component| component.to_str()),
            );
        let Some(pair_root) = path.parent().and_then(Path::parent) else {
            continue;
        };
        let counterpart = match components {
            Some((scripts, source))
                if scripts.eq_ignore_ascii_case("scripts")
                    && source.eq_ignore_ascii_case("source") =>
            {
                pair_root.join("scripts/source")
            }
            Some((source, scripts))
                if source.eq_ignore_ascii_case("source")
                    && scripts.eq_ignore_ascii_case("scripts") =>
            {
                pair_root.join("source/scripts")
            }
            _ => continue,
        };

        if !resolved.contains(&counterpart) {
            resolved.push(counterpart);
        }
    }

    resolved
}

/// Returns the existing source directories that will be searched for scripts.
/// Conventional project directories come first, followed by configured roots.
pub fn detected_script_roots(root: &Path, additional_roots: &[String]) -> Vec<PathBuf> {
    CANDIDATE_DIRS
        .iter()
        .map(|dir| root.join(dir))
        .chain(resolve_additional_roots(root, additional_roots))
        .filter(|path| path.is_dir())
        .collect()
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

        let result = find_psc_file(root.path(), "Foo.psc", &[]);

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn finds_case_insensitive_match_in_source_scripts() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = root.path().join("source/scripts");
        fs::create_dir_all(&source_dir).expect("failed to create source dir");
        let expected = write_file(&source_dir, "Foo.psc");

        let result = find_psc_file(root.path(), "fOO.PSC", &[]);

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn appends_extension_when_omitted() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = root.path().join("scripts/source");
        fs::create_dir_all(&source_dir).expect("failed to create source dir");
        let expected = write_file(&source_dir, "Foo.psc");

        let result = find_psc_file(root.path(), "foo", &[]);

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

        let result = find_psc_file(root.path(), "Foo.psc", &[]);

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn returns_none_when_no_match_found() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = root.path().join("scripts/source");
        fs::create_dir_all(&source_dir).expect("failed to create source dir");
        write_file(&source_dir, "Bar.psc");

        let result = find_psc_file(root.path(), "Foo.psc", &[]);

        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_when_neither_directory_exists() {
        let root = tempfile::tempdir().expect("failed to create temp dir");

        let result = find_psc_file(root.path(), "Foo.psc", &[]);

        assert_eq!(result, None);
    }

    #[test]
    fn finds_match_in_an_additional_root_relative_to_the_project_root() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let shared_dir = root.path().join("../SharedScripts");
        fs::create_dir_all(&shared_dir).expect("failed to create shared dir");
        write_file(&shared_dir, "Foo.psc");

        let result = find_psc_file(root.path(), "Foo.psc", &["../SharedScripts".to_string()]);

        assert!(result.is_some());
    }

    #[test]
    fn finds_match_in_an_absolute_additional_root() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let shared = tempfile::tempdir().expect("failed to create temp dir");
        let expected = write_file(shared.path(), "Foo.psc");

        let result = find_psc_file(
            root.path(),
            "Foo.psc",
            &[shared.path().to_string_lossy().into_owned()],
        );

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn prefers_candidate_dirs_over_additional_roots() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let scripts_source = root.path().join("scripts/source");
        let shared = tempfile::tempdir().expect("failed to create temp dir");
        fs::create_dir_all(&scripts_source).expect("failed to create scripts/source dir");
        let expected = write_file(&scripts_source, "Foo.psc");
        write_file(shared.path(), "Foo.psc");

        let result = find_psc_file(
            root.path(),
            "Foo.psc",
            &[shared.path().to_string_lossy().into_owned()],
        );

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn searches_additional_roots_in_order() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let first = tempfile::tempdir().expect("failed to create temp dir");
        let second = tempfile::tempdir().expect("failed to create temp dir");
        let expected = write_file(first.path(), "Foo.psc");
        write_file(second.path(), "Foo.psc");

        let result = find_psc_file(
            root.path(),
            "Foo.psc",
            &[
                first.path().to_string_lossy().into_owned(),
                second.path().to_string_lossy().into_owned(),
            ],
        );

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn searches_scripts_source_beside_an_added_source_scripts_root() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let scripts_source = root.path().join("shared/scripts/source");
        fs::create_dir_all(&scripts_source).expect("failed to create scripts/source dir");
        let expected = write_file(&scripts_source, "Foo.psc");

        let result = find_psc_file(root.path(), "Foo.psc", &["shared/source/scripts".into()]);

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn searches_source_scripts_beside_an_added_scripts_source_root() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source_scripts = root.path().join("shared/source/scripts");
        fs::create_dir_all(&source_scripts).expect("failed to create source/scripts dir");
        let expected = write_file(&source_scripts, "Foo.psc");

        let result = find_psc_file(root.path(), "Foo.psc", &["shared/scripts/source".into()]);

        assert_eq!(result, Some(expected));
    }

    #[test]
    fn resolve_additional_roots_joins_relative_and_keeps_absolute_paths() {
        let root = Path::new("/game/Data");

        let resolved = resolve_additional_roots(
            root,
            &[
                "../SharedScripts".to_string(),
                "/abs/OtherScripts".to_string(),
            ],
        );

        assert_eq!(
            resolved,
            vec![
                root.join("../SharedScripts"),
                PathBuf::from("/abs/OtherScripts"),
            ]
        );
    }

    #[test]
    fn resolve_additional_roots_adds_counterparts_without_duplicates() {
        let root = Path::new("/game/Data");

        let resolved = resolve_additional_roots(
            root,
            &[
                "Shared/source/scripts".to_string(),
                "Shared/scripts/source".to_string(),
            ],
        );

        assert_eq!(
            resolved,
            vec![
                root.join("Shared/source/scripts"),
                root.join("Shared/scripts/source"),
            ]
        );
    }

    #[test]
    fn detected_script_roots_returns_only_existing_search_directories() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let conventional = root.path().join("scripts/source");
        let additional = root.path().join("shared");
        fs::create_dir_all(&conventional).expect("failed to create conventional root");
        fs::create_dir_all(&additional).expect("failed to create additional root");

        assert_eq!(
            detected_script_roots(root.path(), &["shared".to_string(), "missing".to_string()]),
            vec![conventional, additional]
        );
    }
}
