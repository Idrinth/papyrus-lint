//! Re-exports the project-root discovery logic shared with the desktop
//! app's port of the same algorithm (see
//! [`papyrus_lint_core::project_root`]), kept here so `crate::project::*`
//! stays valid for the rest of this crate.

use std::path::{Path, PathBuf};

pub(crate) use papyrus_lint_core::project_root::{
    display_path, find_candidate_pair_root, find_psc_project_root,
};

/// Whether `path` looks like a Papyrus source file, matching how `run`,
/// `doctor`, and `scan_project` decide between a bare `.psc`, a directory,
/// and an `.achlist`.
pub(crate) fn is_psc_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"))
}

/// Resolves the project root a lint/fix/`doctor` run should use for
/// `input_path`, mirroring the walk documented on [`crate::run`]: a bare
/// `.psc` walks up from the file itself; an `.achlist` or scanned
/// directory tries each resolved script first, then falls back to the
/// directory itself (scan) or the achlist's parent.
pub(crate) fn resolve_input_project_root(
    input_path: &Path,
    script_paths: &[PathBuf],
    is_psc_file: bool,
    is_directory: bool,
) -> PathBuf {
    if is_psc_file {
        return find_psc_project_root(input_path);
    }
    script_paths
        .iter()
        .find_map(|path| find_candidate_pair_root(path))
        .unwrap_or_else(|| {
            if is_directory {
                input_path.to_path_buf()
            } else {
                input_path
                    .ancestors()
                    .nth(1)
                    .filter(|dir| !dir.as_os_str().is_empty())
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            }
        })
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
