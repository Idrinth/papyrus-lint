//! Locates Papyrus `.psc` source files by case-insensitive name.

use std::fs;
use std::path::{Path, PathBuf};

use papyrus_lints::Diagnostic;

/// Rule id used when multiple search roots contain different versions of
/// the same script.
pub const CONFLICTING_SCRIPT_VERSIONS_RULE: &str = "conflicting-script-versions";

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
/// one of them. Paths that are not existing directories are omitted. Used to
/// turn a project's
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

        if path.is_dir() && !resolved.contains(&path) {
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

        if counterpart.is_dir() && !resolved.contains(&counterpart) {
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

/// Warns when `script_path` has a same-named, byte-different counterpart in
/// another script search directory.
///
/// Papyrus resolves a script by search-root precedence, so having multiple
/// versions available makes the source used by the compiler dependent on its
/// import-directory ordering. Identical copies are harmless and are ignored.
pub fn conflicting_script_versions(
    script_path: &Path,
    root: &Path,
    additional_roots: &[String],
) -> Vec<Diagnostic> {
    let Some(file_name) = script_path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let Ok(current) = fs::read(script_path) else {
        return Vec::new();
    };
    let current_hash = md5::compute(&current);

    let mut conflicts = Vec::new();
    for search_root in detected_script_roots(root, additional_roots) {
        let Ok(entries) = fs::read_dir(search_root) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path();
            let same_name = candidate
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case(file_name));
            if !same_name || !candidate.is_file() || candidate == script_path {
                continue;
            }
            let Ok(contents) = fs::read(&candidate) else {
                continue;
            };
            if md5::compute(contents) != current_hash && !conflicts.contains(&candidate) {
                conflicts.push(candidate);
            }
        }
    }

    conflicts.sort();
    conflicts
        .into_iter()
        .map(|path| Diagnostic {
            line: 1,
            column: 1,
            rule: CONFLICTING_SCRIPT_VERSIONS_RULE,
            message: format!(
                "[warning] A different version of {} is also available at {}; script resolution may depend on search-directory order",
                file_name,
                path.display()
            ),
        })
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
    fn ignores_a_directory_with_the_target_filename() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = root.path().join("scripts/source");
        fs::create_dir_all(source_dir.join("Foo.psc"))
            .expect("failed to create misleading directory");

        let result = find_psc_file(root.path(), "Foo.psc", &[]);

        assert_eq!(result, None);
    }

    #[test]
    fn skips_non_directory_search_roots() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let additional_root = root.path().join("not-a-directory");
        fs::write(&additional_root, "content").expect("failed to create regular file");

        let result = find_psc_file(
            root.path(),
            "Foo.psc",
            &[additional_root.to_string_lossy().into_owned()],
        );

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
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let relative = root.path().join("../SharedScripts");
        let absolute = tempfile::tempdir().expect("failed to create absolute root");
        fs::create_dir_all(&relative).expect("failed to create relative root");

        let resolved = resolve_additional_roots(
            root.path(),
            &[
                "../SharedScripts".to_string(),
                absolute.path().to_string_lossy().into_owned(),
            ],
        );

        assert_eq!(resolved, vec![relative, absolute.path().to_path_buf()]);
    }

    #[test]
    fn resolve_additional_roots_adds_counterparts_without_duplicates() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source_scripts = root.path().join("Shared/source/scripts");
        let scripts_source = root.path().join("Shared/scripts/source");
        fs::create_dir_all(&source_scripts).expect("failed to create source/scripts dir");
        fs::create_dir_all(&scripts_source).expect("failed to create scripts/source dir");

        let resolved = resolve_additional_roots(
            root.path(),
            &[
                "Shared/source/scripts".to_string(),
                "Shared/scripts/source".to_string(),
            ],
        );

        assert_eq!(resolved, vec![source_scripts, scripts_source]);
    }

    #[test]
    fn resolve_additional_roots_omits_nonexistent_directories() {
        let root = tempfile::tempdir().expect("failed to create temp dir");

        assert!(resolve_additional_roots(root.path(), &["missing".to_string()]).is_empty());
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

    #[test]
    fn detected_script_roots_excludes_regular_files() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let regular_file = root.path().join("shared");
        fs::write(&regular_file, "content").expect("failed to create regular file");

        assert!(detected_script_roots(root.path(), &["shared".to_string()]).is_empty());
    }

    #[test]
    fn warns_about_same_named_scripts_with_different_contents() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let primary = root.path().join("scripts/source");
        let alternate = root.path().join("source/scripts");
        fs::create_dir_all(&primary).expect("failed to create primary root");
        fs::create_dir_all(&alternate).expect("failed to create alternate root");
        let script = write_file(&primary, "Example.psc");
        fs::write(&script, "ScriptName Example\n").expect("failed to write primary script");
        fs::write(alternate.join("example.PSC"), "ScriptName ExampleV2\n")
            .expect("failed to write alternate script");

        let diagnostics = conflicting_script_versions(&script, root.path(), &[]);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule, CONFLICTING_SCRIPT_VERSIONS_RULE);
        assert!(diagnostics[0].message.contains("example.PSC"));
    }

    #[test]
    fn ignores_identical_same_named_scripts() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let primary = root.path().join("scripts/source");
        let alternate = root.path().join("source/scripts");
        fs::create_dir_all(&primary).expect("failed to create primary root");
        fs::create_dir_all(&alternate).expect("failed to create alternate root");
        let script = write_file(&primary, "Example.psc");
        fs::write(&script, "same").expect("failed to write primary script");
        fs::write(alternate.join("Example.psc"), "same")
            .expect("failed to write alternate script");

        assert!(conflicting_script_versions(&script, root.path(), &[]).is_empty());
    }
}
