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
#[path = "project_root_tests.rs"]
mod tests;
