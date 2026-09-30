//! Project settings for the Papyrus compiler integration.

use std::path::Path;

use crate::project_file::{load_project_file, load_project_file_from_path, save_project_file};

/// Reads `dir`'s papyrus-lint config file and returns the explicit
/// PapyrusCompiler.exe path override it stores, if any (an empty string is
/// treated the same as no override).
pub fn load_compiler_path(dir: &Path) -> Result<Option<String>, String> {
    Ok(trimmed_compiler_path(
        load_project_file(dir)?.compiler_path,
    ))
}

/// Reads an explicit config file at `path` (see [`crate::load_config_from_path`])
/// and returns the PapyrusCompiler.exe override it stores, the same way
/// [`load_compiler_path`] does for a project directory's own
/// papyrus-lint.yaml/.yml. Used so a `--config <path>` override still
/// honors `compiler_path` from the file it names.
pub fn load_compiler_path_from_path(path: &Path) -> Result<Option<String>, String> {
    Ok(trimmed_compiler_path(
        load_project_file_from_path(path)?.compiler_path,
    ))
}

fn trimmed_compiler_path(path: Option<String>) -> Option<String> {
    path.filter(|path| !path.trim().is_empty())
}

/// Persists an explicit PapyrusCompiler.exe path override to `dir`'s
/// papyrus-lint config file, preserving its lint settings. `path: None`
/// (or an empty string) clears the override, reverting to auto-detection.
pub fn save_compiler_path(dir: &Path, path: Option<&str>) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.compiler_path = path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned);
    save_project_file(dir, &project)
}

/// Reads `dir`'s papyrus-lint config file and returns whether it enables
/// running PapyrusCompiler.exe as part of linting a `.psc`, surfacing any
/// errors it reports alongside the lint engine's own findings. `false`
/// (the default) if `dir` has no config file or doesn't set the key.
pub fn load_compile_check(dir: &Path) -> Result<bool, String> {
    Ok(load_project_file(dir)?.compile_check)
}

/// Reads an explicit config file at `path` (see [`crate::load_config_from_path`])
/// and returns whether it enables `compile_check`, the same way
/// [`load_compile_check`] does for a project directory's own
/// papyrus-lint.yaml/.yml. Used so a `--config <path>` override still
/// honors the flag from the file it names.
pub fn load_compile_check_from_path(path: &Path) -> Result<bool, String> {
    Ok(load_project_file_from_path(path)?.compile_check)
}

/// Persists whether the desktop app runs PapyrusCompiler.exe as part of
/// linting a `.psc` to `dir`'s papyrus-lint config file, preserving its
/// other settings.
pub fn save_compile_check(dir: &Path, enabled: bool) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.compile_check = enabled;
    save_project_file(dir, &project)
}

#[cfg(test)]
#[path = "compiler_config_tests/mod.rs"]
mod tests;
