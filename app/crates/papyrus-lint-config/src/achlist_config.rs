//! Project settings that control `.achlist` scoping.

use std::fs;
use std::path::Path;

use crate::project_file::{load_project_file, project_file_from_yaml};

/// Reads whether `dir` scopes cross-script resolution and
/// `conflicting_script_versions` strictly to an
/// `.achlist`'s own listed entries, rather than treating every listed
/// entry's parent directory as a generic search root. `false` (the
/// default, preserving the resolution an achlist project may already
/// depend on) if `dir` has no config file or doesn't set the key.
pub fn load_strict_achlist_scope(dir: &Path) -> Result<bool, String> {
    Ok(load_project_file(dir)?.strict_achlist_scope)
}

/// Reads an explicit config file at `path` (see [`load_config_from_path`])
/// and returns whether it sets `strict_achlist_scope`, the same way
/// [`load_strict_achlist_scope`] does for a project directory's own
/// papyrus-lint.yaml/.yml. Used so a `--config <path>` override still
/// honors the flag from the file it explicitly names, instead of that file
/// being read only for its `papyrus_lints::Config` fields. Returns an
/// error if `path` doesn't exist or fails to parse.
pub fn load_strict_achlist_scope_from_path(path: &Path) -> Result<bool, String> {
    let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
    if contents.trim().is_empty() {
        return Ok(false);
    }
    Ok(project_file_from_yaml(&contents)?.strict_achlist_scope)
}

#[cfg(test)]
#[path = "achlist_config_tests.rs"]
mod tests;
