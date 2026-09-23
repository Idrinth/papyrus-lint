//! Discovers a Papyrus project's root directory from the position of a
//! `.psc` file on disk, and formats paths relative to it for display.
//!
//! Shared by the CLI (a bare `.psc` file, or each entry resolved from an
//! `.achlist`/scanned directory — see `papyrus-lint-cli`'s `run`) and, via
//! the `find_project_root`/`find_psc_project_root_for_path` Tauri commands
//! (`app/src-tauri/src/project_root.rs`), the desktop app's own drop
//! handling (`app/src/project.ts`'s `projectDirForAchlist`/
//! `projectDirForDirectory`/`projectDirForPscPath`) — so both land on the
//! same project root, and therefore the same `papyrus-lint.yaml` and
//! cross-script `FunctionTable` resolution, for the same files.

use std::path::{Path, PathBuf};

use crate::script_locator::CANDIDATE_DIRS;

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
/// `.achlist` file or a recursively scanned directory — so a project whose
/// `.achlist`/scanned directory isn't the real project root (e.g. it was
/// dropped next to a game's `Data` directory while the project itself lives
/// in a subfolder) still resolves to the right root, as long as at least
/// one of its entries sits under a conventionally-named
/// `scripts/source`/`source/scripts` tree.
pub fn find_candidate_pair_root(psc_path: &Path) -> Option<PathBuf> {
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

/// Walks up `psc_path`'s ancestors, starting at its parent directory,
/// looking for one that already has a `papyrus-lint.yaml`/`.yml` config
/// file (see [`papyrus_lint_config::config_file_path`]), and returns the
/// first such ancestor.
///
/// This is [`find_psc_project_root`]'s first choice: an actual config file
/// on disk beats the `scripts/source`/`source/scripts` naming guess, so a
/// `papyrus-lint.yaml` placed at a project's real root — e.g. above a
/// `Data` folder whose own `Scripts/Source` pair would otherwise make
/// [`find_candidate_pair_root`] land one directory too low — still gets
/// found and applied, rather than editor plugins (VS Code, Sublime) silently
/// linting against the built-in defaults because their single-file
/// invocation walked past it. Checks the filesystem directly rather than
/// just path components, unlike [`find_candidate_pair_root`], since there's
/// no naming convention here to match against.
fn find_config_file_root(psc_path: &Path) -> Option<PathBuf> {
    psc_path
        .ancestors()
        .skip(1)
        .find(|dir| papyrus_lint_config::config_file_path(dir).is_some())
        .map(Path::to_path_buf)
}

/// Finds the project root for a bare `.psc` file given directly on the
/// command line.
///
/// Tries [`find_config_file_root`] first — an actual `papyrus-lint.yaml`/
/// `.yml` found anywhere in `psc_path`'s ancestry wins, since that's the
/// project's real, explicit root — then falls back to
/// [`find_candidate_pair_root`]'s `scripts/source`/`source/scripts` naming
/// guess when no config file exists anywhere above the script, e.g. a
/// project laid out some other way (Requiem's own, arbitrarily nested
/// layout) that hasn't added a config file yet. Only falls back to the
/// previous fixed "two directories up" guess if neither finds anything,
/// e.g. a `.psc` with no project config anywhere in its ancestry at all and
/// no recognizable `scripts/source` pair either.
pub fn find_psc_project_root(psc_path: &Path) -> PathBuf {
    find_config_file_root(psc_path)
        .or_else(|| find_candidate_pair_root(psc_path))
        .unwrap_or_else(|| {
            psc_path
                .ancestors()
                .nth(3)
                .filter(|dir| !dir.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        })
}

/// Formats `path` for display: with `short_paths` set, strips
/// `project_root` from its beginning, mirroring how the desktop app
/// shortens paths in its own results list (see `relativePath` in
/// `app/src/main.ts`); otherwise, or when `path` doesn't sit under
/// `project_root`, returns it unchanged.
pub fn display_path(path: &Path, project_root: &Path, short_paths: bool) -> String {
    if short_paths {
        if let Ok(stripped) = path.strip_prefix(project_root) {
            return stripped.display().to_string();
        }
    }
    path.display().to_string()
}

#[cfg(test)]
#[path = "project_root_tests.rs"]
mod tests;
