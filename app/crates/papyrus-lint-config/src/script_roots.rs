//! Loading and saving additional and analysis-only script roots.

use std::path::Path;

use crate::project_file::{load_project_file, load_project_file_from_path, save_project_file};

/// Reads `dir`'s additional script-root directories, which are used alongside
/// the conventional `scripts/source` and `source/scripts` directories to
/// resolve cross-script lookups and the compiler's `-i`
/// argument). Empty (or blank) entries are dropped. Returns an empty `Vec`
/// if `dir` has no config file or it declares none.
pub fn load_script_roots(dir: &Path) -> Result<Vec<String>, String> {
    let roots = load_project_file(dir)?.additional_script_roots;
    Ok(roots
        .into_iter()
        .map(|root| root.trim().to_string())
        .filter(|root| !root.is_empty())
        .collect())
}

/// Persists `roots` as `dir`'s papyrus-lint config file's additional script
/// root directories, preserving its lint settings and compiler path
/// override. Empty (or blank) entries are dropped before saving.
pub fn save_script_roots(dir: &Path, roots: &[String]) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.additional_script_roots = roots
        .iter()
        .map(|root| root.trim().to_string())
        .filter(|root| !root.is_empty())
        .collect();
    save_project_file(dir, &project)
}

/// Reads `dir`'s papyrus-lint config file and returns the analysis-only
/// lookup directories it lists (see [`crate::script_locator`] /
/// [`crate::function_table::FunctionTable::with_lookup_roots`]). These are
/// searched only after the conventional and `additional_script_roots`
/// directories, never linted, and never considered by
/// `conflicting_script_versions`. Empty (or blank) entries are dropped.
pub fn load_lookup_script_roots(dir: &Path) -> Result<Vec<String>, String> {
    Ok(trimmed_roots(load_project_file(dir)?.lookup_script_roots))
}

/// Reads an explicit config file at `path` (see [`load_config_from_path`])
/// and returns its `lookup_script_roots`, the same way
/// [`load_lookup_script_roots`] does for a project directory's own
/// papyrus-lint.yaml/.yml. Used so a `--config <path>` override still
/// honors analysis-only lookup directories from the file it names.
pub fn load_lookup_script_roots_from_path(path: &Path) -> Result<Vec<String>, String> {
    Ok(trimmed_roots(
        load_project_file_from_path(path)?.lookup_script_roots,
    ))
}

/// Persists `roots` as `dir`'s papyrus-lint config file's analysis-only
/// lookup directories, preserving its other settings. Empty (or blank)
/// entries are dropped.
pub fn save_lookup_script_roots(dir: &Path, roots: &[String]) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.lookup_script_roots = trimmed_roots(roots.iter().cloned());
    save_project_file(dir, &project)
}

fn trimmed_roots(roots: impl IntoIterator<Item = String>) -> Vec<String> {
    roots
        .into_iter()
        .map(|root| root.trim().to_string())
        .filter(|root| !root.is_empty())
        .collect()
}

#[cfg(test)]
#[path = "script_roots_tests/mod.rs"]
mod tests;
