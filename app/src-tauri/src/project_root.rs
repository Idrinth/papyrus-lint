//! Project-root discovery commands, delegating to the same algorithm the
//! CLI uses (see `papyrus_lint_core::project_root`) so a project dropped on
//! the desktop app resolves to the same root, and therefore the same
//! `papyrus-lint.yaml` and cross-script lookups, as the CLI would for the
//! same files.

use std::path::Path;

use papyrus_lint_core::project_root::{find_candidate_pair_root, find_psc_project_root};

/// Finds the project root implied by `entries`: each is checked in turn
/// (via [`find_candidate_pair_root`]) until one sits under a
/// `scripts/source`/`source/scripts` pair, returning the directory above
/// that pair. Falls back to `fallback` if none of `entries` match — e.g.
/// an achlist/directory drop whose scripts don't follow that convention.
#[tauri::command(async)]
pub(crate) fn find_project_root(entries: Vec<String>, fallback: String) -> String {
    entries
        .iter()
        .find_map(|entry| find_candidate_pair_root(Path::new(entry)))
        .map(|root| root.to_string_lossy().into_owned())
        .unwrap_or(fallback)
}

/// Finds the project root for a bare `.psc` file dropped directly, per
/// [`find_psc_project_root`].
#[tauri::command(async)]
pub(crate) fn find_psc_project_root_for_path(path: String) -> String {
    find_psc_project_root(Path::new(&path))
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn project_root_uses_the_first_entry_in_a_supported_script_tree() {
        let dir = tempdir().unwrap();
        let first_root = dir.path().join("first-project");
        let second_root = dir.path().join("second-project");
        let first_script = first_root.join("scripts/source/nested/First.psc");
        let second_script = second_root.join("source/scripts/Second.psc");

        assert_eq!(
            find_project_root(
                vec![
                    dir.path()
                        .join("unmatched/Example.psc")
                        .to_string_lossy()
                        .into_owned(),
                    first_script.to_string_lossy().into_owned(),
                    second_script.to_string_lossy().into_owned(),
                ],
                "fallback".to_string(),
            ),
            first_root.to_string_lossy()
        );
    }

    #[test]
    fn project_root_returns_the_fallback_when_no_entry_matches() {
        assert_eq!(
            find_project_root(
                vec!["custom/source/Example.psc".to_string()],
                "selected/project".to_string(),
            ),
            "selected/project"
        );
        assert_eq!(
            find_project_root(Vec::new(), "empty/project".to_string()),
            "empty/project"
        );
    }

    #[test]
    fn psc_project_root_command_handles_conventional_and_fallback_layouts() {
        let separator = std::path::MAIN_SEPARATOR;

        assert_eq!(
            find_psc_project_root_for_path(format!(
                "project{separator}Scripts{separator}Source{separator}nested{separator}Example.psc"
            )),
            "project"
        );
        assert_eq!(
            find_psc_project_root_for_path(format!(
                "project{separator}custom{separator}source{separator}Example.psc"
            )),
            "project"
        );
        assert_eq!(
            find_psc_project_root_for_path("Example.psc".to_string()),
            "."
        );
    }
}
