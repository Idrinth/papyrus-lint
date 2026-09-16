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
