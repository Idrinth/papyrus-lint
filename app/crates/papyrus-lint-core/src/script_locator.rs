//! Locates Papyrus `.psc` source files by case-insensitive name.

use std::collections::HashMap;
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

/// Maps a script file name (case-insensitively lowercased, as returned by
/// [`detected_script_roots`]'s directories) to every path found under those
/// directories carrying that name. Built once by [`build_script_index`] so
/// both exact-name resolution and conflict checks over a whole batch of
/// scripts (e.g. an achlist's worth) can avoid re-scanning the same roots.
pub type ScriptIndex = HashMap<String, Vec<PathBuf>>;

/// Looks up `name` in a pre-built [`ScriptIndex`], returning the first path
/// in search-root order. `name` may be supplied with or without `.psc` and
/// is matched case-insensitively, just like [`find_psc_file`].
pub fn find_psc_file_in_index(index: &ScriptIndex, name: &str) -> Option<PathBuf> {
    let name_lower = name.to_ascii_lowercase();
    let target = if name_lower.ends_with(".psc") {
        name_lower
    } else {
        format!("{name_lower}.psc")
    };

    index.get(&target).and_then(|paths| paths.first()).cloned()
}

/// Scans `root`'s conventional and configured search directories (see
/// [`detected_script_roots`]) once, recording every `.psc` file found under
/// them by lowercased file name. Reuse the result with
/// [`find_psc_file_in_index`] for name resolution and
/// [`conflicting_script_versions_in_index`] for each script being checked.
pub fn build_script_index(root: &Path, additional_roots: &[String]) -> ScriptIndex {
    let mut index: ScriptIndex = HashMap::new();

    for search_root in detected_script_roots(root, additional_roots) {
        let Ok(entries) = fs::read_dir(search_root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let bucket = index.entry(file_name.to_ascii_lowercase()).or_default();
            if !bucket.contains(&path) {
                bucket.push(path);
            }
        }
    }

    index
}

/// Like [`conflicting_script_versions`], but checks `script_path` against a
/// pre-built [`ScriptIndex`] (see [`build_script_index`]) instead of
/// scanning `root`'s search directories itself. Use this when checking many
/// scripts from the same project in one run, so the directories are only
/// scanned once for the whole batch rather than once per script.
pub fn conflicting_script_versions_in_index(
    script_path: &Path,
    index: &ScriptIndex,
) -> Vec<Diagnostic> {
    let Some(file_name) = script_path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let Some(candidates) = index.get(&file_name.to_ascii_lowercase()) else {
        return Vec::new();
    };

    conflicting_script_versions_among(script_path, candidates)
}

/// Warns when `script_path` has a same-named, byte-different counterpart
/// among `known_scripts` — other scripts named explicitly (e.g. an
/// `.achlist`'s own entries) rather than discovered by directory search.
///
/// Complements [`conflicting_script_versions`], which only scans `root`'s
/// conventional and configured search directories: two `.achlist` entries
/// can share a file name while living in directories that were never (and,
/// to keep resolution scoped to what was actually listed, deliberately
/// aren't) treated as search roots for one another. `known_scripts` should
/// generally be pre-filtered to only the paths sharing `script_path`'s file
/// name (see [`crate::function_table::FunctionTable::with_known_scripts`]
/// for the matching resolution side of this), so checking every script in a
/// large achlist stays proportional to how many of them actually collide by
/// name rather than to the achlist's full size.
pub fn conflicting_script_versions_among(
    script_path: &Path,
    known_scripts: &[PathBuf],
) -> Vec<Diagnostic> {
    let Some(file_name) = script_path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let Ok(current) = fs::read(script_path) else {
        return Vec::new();
    };
    let current_hash = md5::compute(&current);

    let mut conflicts = Vec::new();
    for candidate in known_scripts {
        let same_name = candidate
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(file_name));
        if !same_name || candidate == script_path {
            continue;
        }
        let Ok(contents) = fs::read(candidate) else {
            continue;
        };
        if md5::compute(contents) != current_hash && !conflicts.contains(candidate) {
            conflicts.push(candidate.clone());
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
        fs::write(alternate.join("Example.psc"), "same").expect("failed to write alternate script");

        assert!(conflicting_script_versions(&script, root.path(), &[]).is_empty());
    }

    #[test]
    fn reports_conflicts_from_additional_roots_in_sorted_path_order() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let primary = root.path().join("scripts/source");
        let first_alphabetically = root.path().join("a-scripts");
        let second_alphabetically = root.path().join("z-scripts");
        fs::create_dir_all(&primary).expect("failed to create primary root");
        fs::create_dir_all(&first_alphabetically).expect("failed to create first root");
        fs::create_dir_all(&second_alphabetically).expect("failed to create second root");
        let script = primary.join("Example.psc");
        fs::write(&script, "primary").expect("failed to write primary script");
        fs::write(first_alphabetically.join("example.psc"), "first")
            .expect("failed to write first alternate");
        fs::write(second_alphabetically.join("EXAMPLE.PSC"), "second")
            .expect("failed to write second alternate");

        let diagnostics = conflicting_script_versions(
            &script,
            root.path(),
            &["z-scripts".into(), "a-scripts".into()],
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0]
            .message
            .contains(&first_alphabetically.display().to_string()));
        assert!(diagnostics[1]
            .message
            .contains(&second_alphabetically.display().to_string()));
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.line == 1 && diagnostic.column == 1));
    }

    #[test]
    fn conflict_check_returns_empty_for_an_unreadable_script_path() {
        let root = tempfile::tempdir().expect("failed to create temp dir");

        assert!(
            conflicting_script_versions(&root.path().join("Missing.psc"), root.path(), &[])
                .is_empty()
        );
    }

    #[test]
    fn conflict_check_ignores_same_named_directories() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let primary = root.path().join("scripts/source");
        let alternate = root.path().join("source/scripts");
        fs::create_dir_all(&primary).expect("failed to create primary root");
        fs::create_dir_all(alternate.join("Example.psc"))
            .expect("failed to create same-named directory");
        let script = primary.join("Example.psc");
        fs::write(&script, "primary").expect("failed to write primary script");

        assert!(conflicting_script_versions(&script, root.path(), &[]).is_empty());
    }

    #[test]
    fn build_script_index_groups_files_by_lowercased_name_across_search_roots() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let primary = root.path().join("scripts/source");
        let alternate = root.path().join("source/scripts");
        fs::create_dir_all(&primary).expect("failed to create primary root");
        fs::create_dir_all(&alternate).expect("failed to create alternate root");
        let script = write_file(&primary, "Example.psc");
        let other = write_file(&alternate, "example.PSC");
        write_file(&primary, "Unrelated.psc");

        let index = build_script_index(root.path(), &[]);

        let mut matches = index
            .get("example.psc")
            .expect("expected an entry for example.psc")
            .clone();
        matches.sort();
        let mut expected = vec![script, other];
        expected.sort();
        assert_eq!(matches, expected);
    }

    #[test]
    fn indexed_lookup_is_case_insensitive_and_preserves_search_order() {
        let first = PathBuf::from("first/Example.psc");
        let second = PathBuf::from("second/example.PSC");
        let index = HashMap::from([("example.psc".to_string(), vec![first.clone(), second])]);

        assert_eq!(find_psc_file_in_index(&index, "EXAMPLE"), Some(first));
        assert_eq!(find_psc_file_in_index(&index, "missing.psc"), None);
    }

    #[test]
    fn build_script_index_ignores_directories_and_missing_search_roots() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let primary = root.path().join("scripts/source");
        fs::create_dir_all(primary.join("Example.psc"))
            .expect("failed to create same-named directory");

        let index = build_script_index(root.path(), &[]);

        assert!(!index.contains_key("example.psc"));
    }

    #[test]
    fn conflicting_script_versions_in_index_flags_a_same_named_entry_with_different_content() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let primary = root.path().join("scripts/source");
        let alternate = root.path().join("source/scripts");
        fs::create_dir_all(&primary).expect("failed to create primary root");
        fs::create_dir_all(&alternate).expect("failed to create alternate root");
        let script = write_file(&primary, "Example.psc");
        fs::write(&script, "ScriptName Example\n").expect("failed to write primary script");
        fs::write(alternate.join("example.PSC"), "ScriptName ExampleV2\n")
            .expect("failed to write alternate script");
        let index = build_script_index(root.path(), &[]);

        let diagnostics = conflicting_script_versions_in_index(&script, &index);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule, CONFLICTING_SCRIPT_VERSIONS_RULE);
        assert!(diagnostics[0].message.contains("example.PSC"));
    }

    #[test]
    fn conflicting_script_versions_in_index_matches_conflicting_script_versions() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let primary = root.path().join("scripts/source");
        let alternate = root.path().join("source/scripts");
        fs::create_dir_all(&primary).expect("failed to create primary root");
        fs::create_dir_all(&alternate).expect("failed to create alternate root");
        let script = write_file(&primary, "Example.psc");
        fs::write(&script, "ScriptName Example\n").expect("failed to write primary script");
        fs::write(alternate.join("Example.psc"), "ScriptName ExampleV2\n")
            .expect("failed to write alternate script");
        let index = build_script_index(root.path(), &[]);

        assert_eq!(
            conflicting_script_versions_in_index(&script, &index),
            conflicting_script_versions(&script, root.path(), &[])
        );
    }

    #[test]
    fn conflicting_script_versions_in_index_ignores_unknown_file_names() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let index = build_script_index(root.path(), &[]);
        let script = root.path().join("Untracked.psc");

        assert!(conflicting_script_versions_in_index(&script, &index).is_empty());
    }

    #[test]
    fn conflicting_script_versions_among_flags_a_same_named_known_script_with_different_content() {
        let dir_a = tempfile::tempdir().expect("failed to create temp dir");
        let dir_b = tempfile::tempdir().expect("failed to create temp dir");
        let script = write_file(dir_a.path(), "Example.psc");
        fs::write(&script, "ScriptName Example\n").expect("failed to write first script");
        let other = write_file(dir_b.path(), "example.PSC");
        fs::write(&other, "ScriptName ExampleV2\n").expect("failed to write second script");

        let diagnostics = conflicting_script_versions_among(&script, std::slice::from_ref(&other));

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("example.PSC"));
        assert!(diagnostics[0]
            .message
            .contains(&other.display().to_string()));
    }

    #[test]
    fn conflicting_script_versions_among_ignores_identical_known_scripts() {
        let dir_a = tempfile::tempdir().expect("failed to create temp dir");
        let dir_b = tempfile::tempdir().expect("failed to create temp dir");
        let script = write_file(dir_a.path(), "Example.psc");
        fs::write(&script, "same").expect("failed to write first script");
        let other = write_file(dir_b.path(), "Example.psc");
        fs::write(&other, "same").expect("failed to write second script");

        assert!(conflicting_script_versions_among(&script, &[other]).is_empty());
    }

    #[test]
    fn conflicting_script_versions_among_ignores_the_script_itself_and_unrelated_names() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script = write_file(dir.path(), "Example.psc");
        fs::write(&script, "content").expect("failed to write script");
        let unrelated = write_file(dir.path(), "Other.psc");
        fs::write(&unrelated, "different").expect("failed to write unrelated script");

        assert!(
            conflicting_script_versions_among(&script, &[script.clone(), unrelated]).is_empty()
        );
    }

    #[test]
    fn conflicting_script_versions_among_returns_empty_for_an_unreadable_script_path() {
        let root = tempfile::tempdir().expect("failed to create temp dir");

        assert!(
            conflicting_script_versions_among(&root.path().join("Missing.psc"), &[]).is_empty()
        );
    }

    #[test]
    fn conflicting_script_versions_among_ignores_unreadable_candidates() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script = write_file(dir.path(), "Example.psc");
        fs::write(&script, "content").expect("failed to write script");
        let missing = dir.path().join("EXAMPLE.PSC");

        assert!(conflicting_script_versions_among(&script, &[missing]).is_empty());
    }

    #[test]
    fn conflicting_script_versions_among_deduplicates_and_sorts_conflicts() {
        let primary = tempfile::tempdir().expect("failed to create primary dir");
        let alternatives = tempfile::tempdir().expect("failed to create alternatives dir");
        let script = write_file(primary.path(), "Example.psc");
        fs::write(&script, "primary").expect("failed to write primary script");
        let first = write_file(alternatives.path(), "EXAMPLE.PSC");
        fs::write(&first, "first").expect("failed to write first alternative");

        let second_dir = tempfile::tempdir().expect("failed to create second alternative dir");
        let second = write_file(second_dir.path(), "example.psc");
        fs::write(&second, "second").expect("failed to write second alternative");

        let diagnostics =
            conflicting_script_versions_among(&script, &[second.clone(), first.clone(), second]);

        let mut expected = vec![first, second_dir.path().join("example.psc")];
        expected.sort();
        assert_eq!(diagnostics.len(), 2);
        for (diagnostic, path) in diagnostics.iter().zip(expected) {
            assert!(diagnostic.message.contains(&path.display().to_string()));
        }
    }
}
