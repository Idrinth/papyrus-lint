//! Loading, parsing, and saving lint-engine settings.

use std::fs;
use std::path::Path;

use crate::project_file::{
    load_project_file, load_project_file_from_path, project_file_from_yaml, save_project_file,
    save_project_file_at, ProjectFile,
};

/// Parses a YAML config document into a [`papyrus_lints::Config`]. An empty
/// document yields [`papyrus_lints::Config::default`], and omitted keys use
/// their defaults. App-level project keys are accepted and ignored.
pub fn parse_lint_yaml(yaml: &str) -> Result<papyrus_lints::Config, String> {
    Ok(project_file_from_yaml(yaml)?.lint)
}

/// Serializes lint settings into the YAML format read by [`parse_lint_yaml`].
pub fn lint_config_to_yaml(config: &papyrus_lints::Config) -> Result<String, String> {
    serde_norway::to_string(config).map_err(|err| err.to_string())
}

/// Looks for a papyrus-lint config file in `dir` and parses it into a
/// [`papyrus_lints::Config`]. Returns [`papyrus_lints::Config::default`]
/// if `dir` contains none of the candidate file names.
pub fn load_config(dir: &Path) -> Result<papyrus_lints::Config, String> {
    Ok(load_project_file(dir)?.lint)
}

/// Reads and parses an explicit config file at `path`, bypassing the
/// `papyrus-lint.yaml`/`.yml` discovery [`load_config`] does in a project
/// directory. Used for an explicit override (e.g. a `--config` CLI flag,
/// or an editor plugin's configured path) that names a config file
/// directly, which need not be called `papyrus-lint.yaml`/`.yml` or live
/// in the project root. Returns an error if `path` doesn't exist or fails
/// to parse.
pub fn load_config_from_path(path: &Path) -> Result<papyrus_lints::Config, String> {
    let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
    parse_lint_yaml(&contents)
}

/// Writes `config` to `dir`'s papyrus-lint YAML config file, preserving
/// any explicit PapyrusCompiler.exe path override already stored there.
pub fn save_config(dir: &Path, config: &papyrus_lints::Config) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.lint = config.clone();
    save_project_file(dir, &project)
}

/// Writes `config` to the exact file at `path`, bypassing the
/// `papyrus-lint.yaml`/`.yml` discovery [`save_config`] does in a project
/// directory — the save-side counterpart of [`load_config_from_path`], used
/// when the desktop app's user-selected config file override (rather than
/// the project's own auto-detected config file) is in effect. Preserves any
/// other settings (compiler path, additional script roots, ...) already
/// stored in that file; creates the file if `path` doesn't exist yet.
pub fn save_config_at_path(path: &Path, config: &papyrus_lints::Config) -> Result<(), String> {
    let mut project = if path.is_file() {
        load_project_file_from_path(path)?
    } else {
        ProjectFile::default()
    };
    project.lint = config.clone();
    save_project_file_at(path, &project)
}

#[cfg(test)]
#[path = "lint_config_tests/mod.rs"]
mod tests;
