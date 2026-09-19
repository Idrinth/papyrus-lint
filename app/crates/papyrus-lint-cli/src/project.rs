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
/// and an `.achlist`/`.ppj`.
pub(crate) fn is_psc_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"))
}

/// Whether `path` looks like a `.ppj` (Papyrus Project XML) file, matching
/// how `scan_project` decides to parse it with
/// [`papyrus_lint_core::ppj::parse_ppj`] instead of as an `.achlist`.
pub(crate) fn is_ppj_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ppj"))
}

/// Resolves `path` to an absolute path (joined onto the process's current
/// directory when it's relative), stringified for
/// [`papyrus_lint_config::load_script_roots`]'s `additional_script_roots`
/// shape. Doesn't require `path` to actually exist, unlike
/// `fs::canonicalize`, since a `.ppj`'s vendor `<Import>` (e.g. a base
/// game's own vanilla source directory) may not exist on the machine
/// running the lint at all. Used for a `.ppj` input's own `<Import>`
/// entries, which — unlike `additional_script_roots`/`--script-root` — are
/// already resolved relative to the ppj file's own directory rather than
/// the project root [`scan_project`](crate::run_scan::scan_project)/`doctor`
/// settle on, so they need to be absolute before being appended to it.
pub(crate) fn absolutize(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    absolute.to_string_lossy().into_owned()
}

/// Resolves the project root a lint/fix/`doctor` run should use for
/// `input_path`, mirroring the walk documented on [`crate::run`]: a bare
/// `.psc` walks up from the file itself; an `.achlist`, `.ppj`, or scanned
/// directory tries each resolved script first, then falls back to the
/// directory itself (scan) or the achlist's/ppj's parent.
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
