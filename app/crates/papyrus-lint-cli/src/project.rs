use std::path::{Path, PathBuf};

use papyrus_lint_core::script_locator::CANDIDATE_DIRS;

/// Walks up `psc_path`'s ancestors looking for a directory pair matching
/// one of [`CANDIDATE_DIRS`] (`scripts/source` or `source/scripts`,
/// matched case-insensitively), and returns the directory above that pair
/// as the project root, or `None` if no such pair appears anywhere in
/// `psc_path`'s ancestry.
///
/// This finds the right root for a script nested further still, e.g. a
/// namespaced Fallout 4 script at `<root>/Scripts/Source/User/MyScript.psc`
/// — unlike a naive "two directories up" rule, which would land on
/// `Scripts` instead of `<root>` for that layout and then fail to discover
/// the project's config or resolve any cross-script lookups against it.
///
/// Used both for a bare `.psc` file given directly on the command line (see
/// [`find_psc_project_root`]) and, per script, for one resolved from an
/// `.achlist` file (see [`crate::run`]) — so an achlist whose own parent directory
/// isn't the real project root (e.g. it was dropped next to a game's `Data`
/// directory while the project itself lives in a subfolder) still resolves
/// to the right root, as long as at least one of its entries sits under a
/// conventionally-named `scripts/source`/`source/scripts` tree.
pub(crate) fn find_candidate_pair_root(psc_path: &Path) -> Option<PathBuf> {
    let candidate_pairs: Vec<(&str, &str)> = CANDIDATE_DIRS
        .iter()
        .filter_map(|dir| dir.split_once('/'))
        .collect();

    let ancestors: Vec<&Path> = psc_path.ancestors().collect();
    for i in 1..ancestors.len().saturating_sub(1) {
        let inner_name = ancestors[i]
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_ascii_lowercase);
        let outer_name = ancestors[i + 1]
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_ascii_lowercase);
        let (Some(inner_name), Some(outer_name)) = (inner_name, outer_name) else {
            continue;
        };

        let matches_candidate = candidate_pairs
            .iter()
            .any(|(outer, inner)| *outer == outer_name && *inner == inner_name);

        if matches_candidate {
            if let Some(root) = ancestors[i + 1].parent() {
                return Some(root.to_path_buf());
            }
        }
    }

    None
}

/// Finds the project root for a bare `.psc` file given directly on the
/// command line.
///
/// Tries [`find_candidate_pair_root`] first, falling back to the previous
/// fixed "two directories up" behavior when no `scripts/source`/
/// `source/scripts` pair is found in the path at all (e.g. a `.psc` passed
/// from outside any conventionally-named scripts/source tree), so that case
/// is unaffected.
pub(crate) fn find_psc_project_root(psc_path: &Path) -> PathBuf {
    find_candidate_pair_root(psc_path).unwrap_or_else(|| {
        psc_path
            .ancestors()
            .nth(3)
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    })
}

/// Formats `path` for the report: with `short_paths` set, strips
/// `project_root` from its beginning, mirroring how the desktop app
/// shortens paths in its own results list (see `relativePath` in
/// `app/src/main.ts`); otherwise, or when `path` doesn't sit under
/// `project_root`, returns it unchanged.
pub(crate) fn display_path(path: &Path, project_root: &Path, short_paths: bool) -> String {
    if short_paths {
        if let Ok(stripped) = path.strip_prefix(project_root) {
            return stripped.display().to_string();
        }
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn display_path_only_shortens_paths_when_requested() {
        let path = Path::new("project/scripts/source/Example.psc");
        let root = Path::new("project");

        assert_eq!(
            display_path(path, root, true),
            Path::new("scripts/source/Example.psc")
                .display()
                .to_string()
        );
        assert_eq!(display_path(path, root, false), path.display().to_string());
    }

    #[test]
    fn candidate_pair_root_recognizes_every_supported_directory_order() {
        let root = Path::new("project");

        assert_eq!(
            find_candidate_pair_root(&root.join("scripts/source/Example.psc")),
            Some(root.to_path_buf())
        );
        assert_eq!(
            find_candidate_pair_root(&root.join("source/scripts/Example.psc")),
            Some(root.to_path_buf())
        );
    }

    #[test]
    fn candidate_pair_root_is_case_insensitive_and_supports_nested_scripts() {
        let script = Path::new("project/SCRIPTS/Source/User/Example.psc");

        assert_eq!(
            find_candidate_pair_root(script),
            Some(PathBuf::from("project"))
        );
    }

    #[test]
    fn psc_project_root_uses_the_legacy_fallback_without_a_candidate_pair() {
        assert_eq!(
            find_psc_project_root(Path::new("project/custom/source/Example.psc")),
            PathBuf::from("project")
        );
        assert_eq!(
            find_psc_project_root(Path::new("Example.psc")),
            PathBuf::from(".")
        );
    }

    #[test]
    fn short_paths_strips_the_project_root_from_reported_paths() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "--short-paths".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stdout.contains("scripts/source/Example.psc:"));
        assert!(stdout.contains("[trailing-whitespace]"));
        assert!(!stdout.contains(dir.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn short_paths_strips_the_project_root_from_json_paths() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "--json".to_string(),
            "--short-paths".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(
            report["files"][0]["path"],
            "scripts/source/Example.psc".replace('/', std::path::MAIN_SEPARATOR_STR)
        );
    }

    #[test]
    fn without_short_paths_reports_the_full_path() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains(dir.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn directory_scan_finds_the_project_root_from_a_nested_scripts_source_pair() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Requiem/Nested.psc"),
            "ScriptName Nested   \n",
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  trailing_whitespace: false\n",
        );
        let target = dir.path().join("scripts/source");

        let (code, stdout, _stderr) = run_captured(&[target.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found"));
    }

    #[test]
    fn directory_scan_falls_back_to_the_scanned_directory_as_project_root() {
        // No scripts/source or source/scripts pair anywhere in the path, so
        // the scanned directory itself must be used as the project root.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("Nested/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  trailing_whitespace: false\n",
        );

        let (code, stdout, _stderr) = run_captured(&[dir.path().to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found"));
    }

    #[test]
    fn honors_the_project_yaml_config_two_directories_above_a_single_psc_file() {
        // Mirrors the real layout a bare .psc file is found at (e.g. an
        // editor plugin invoking the CLI on a saved file), where the
        // project root sits two directories above the script, at
        // `<root>/scripts/source/Example.psc`.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        write_file(&script_path, "ScriptName Example   \n");
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  trailing_whitespace: false\n",
        );

        let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found"));
    }

    #[test]
    fn finds_the_project_root_for_a_psc_nested_under_a_namespaced_subfolder() {
        // A Fallout 4-style namespaced script, e.g. `ScriptName User:MyScript`
        // stored at `Scripts/Source/User/MyScript.psc`, sits three
        // directories under the project root rather than the conventional
        // two. A naive "two directories up" rule would land on `Scripts`
        // instead of the real root, missing the project's config and
        // breaking every cross-script lookup — this must still find the
        // real root and pick up the config there.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/User/MyScript.psc");
        write_file(&script_path, "ScriptName User:MyScript   \n");
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  trailing_whitespace: false\n",
        );

        let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found"));
    }

    #[test]
    fn finds_the_project_root_from_script_position_when_the_achlist_lives_elsewhere() {
        // Users sometimes drop the .achlist somewhere other than the
        // project root (e.g. next to a game's Data directory) while the
        // actual project, including its papyrus-lint.yaml, lives deeper:
        //
        //   achlist
        //   somefolder/
        //     otherfolder/
        //       papyrus-lint.yaml
        //       scripts/source/AType.psc
        //       source/scripts/BType.psc
        //
        // The achlist's own parent directory (the top-level one) has no
        // config file at all, so the project root must instead be found
        // from the resolved scripts' own position under their
        // scripts/source or source/scripts pair.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let project_root = dir.path().join("somefolder/otherfolder");
        write_file(
            &project_root.join("scripts/source/AType.psc"),
            "ScriptName AType   \n",
        );
        write_file(
            &project_root.join("source/scripts/BType.psc"),
            "ScriptName BType   \n",
        );
        write_file(
            &project_root.join("papyrus-lint.yaml"),
            "rules:\n  trailing_whitespace: false\n",
        );
        write_file(
            &dir.path().join("achlist"),
            r#"["somefolder/otherfolder/scripts/source/AType.psc", "somefolder/otherfolder/source/scripts/BType.psc"]"#,
        );
        let achlist_path = dir.path().join("achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        // If the project root were (wrongly) taken as the achlist's own
        // parent directory, papyrus-lint.yaml wouldn't be found and the
        // trailing-whitespace lint (disabled by that config) would fire on
        // both scripts instead.
        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found in 2 script"));
    }

    #[test]
    fn display_path_leaves_paths_outside_the_project_root_unchanged() {
        let path = Path::new("other-project/scripts/source/Example.psc");

        assert_eq!(
            display_path(path, Path::new("project"), true),
            path.display().to_string()
        );
    }
}
