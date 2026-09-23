//! Project configuration commands: lint YAML, compiler path, script roots.

use std::path::PathBuf;

use papyrus_lint_config as config;
use papyrus_lint_core::script_locator;

#[derive(Debug, PartialEq, serde::Serialize)]
pub(crate) struct ProjectInfo {
    detected_script_roots: Vec<String>,
    used_configuration_file: Option<String>,
}

/// Looks for a papyrus-lint YAML config file in `dir` (conventionally the
/// directory containing the `.achlist` file) and returns the lint
/// configuration it describes, falling back to the default configuration
/// if `dir` has no config file.
#[tauri::command(async)]
pub(crate) fn load_lint_config(dir: String) -> Result<papyrus_lints::Config, String> {
    config::load_config(&PathBuf::from(dir))
}

/// Writes `config` to `dir`'s papyrus-lint YAML config file (creating it,
/// as `papyrus-lint.yaml`, if `dir` has none yet), so the formatting
/// selected in the UI is remembered for next time.
#[tauri::command(async)]
pub(crate) fn save_lint_config(dir: String, config: papyrus_lints::Config) -> Result<(), String> {
    config::save_config(&PathBuf::from(dir), &config)
}

/// Reads and parses the config file at the exact `path` given, bypassing
/// the project directory discovery [`load_lint_config`] does. Backs the
/// Settings tab's "Configuration file" override, letting the user point the
/// app at a specific papyrus-lint.yaml/.yml instead of relying on the one
/// auto-detected next to the dropped `.achlist`/`.psc`.
#[tauri::command(async)]
pub(crate) fn load_lint_config_from_path(path: String) -> Result<papyrus_lints::Config, String> {
    config::load_config_from_path(&PathBuf::from(path))
}

/// Writes `config` to the exact file at `path`, creating it if it doesn't
/// exist yet. The save-side counterpart of [`load_lint_config_from_path`],
/// used while the Settings tab's "Configuration file" override is set.
#[tauri::command(async)]
pub(crate) fn save_lint_config_to_path(
    path: String,
    config: papyrus_lints::Config,
) -> Result<(), String> {
    config::save_config_at_path(&PathBuf::from(path), &config)
}

/// Returns the PapyrusCompiler.exe path to use for `dir`'s project: an
/// explicit override saved to its papyrus-lint config file, or, absent
/// one, a path auto-detected at `../Papyrus Compiler/PapyrusCompiler.exe`
/// relative to `dir` (the directory containing the `.achlist` file).
/// Returns `null` if neither is available.
#[tauri::command(async)]
pub(crate) fn load_compiler_path(dir: String) -> Result<Option<String>, String> {
    config::resolve_compiler_path(&PathBuf::from(dir))
}

/// Persists an explicit PapyrusCompiler.exe path override to `dir`'s
/// papyrus-lint config file. Passing an empty (or blank) string clears
/// the override, reverting to auto-detection.
#[tauri::command(async)]
pub(crate) fn save_compiler_path(dir: String, path: String) -> Result<(), String> {
    let path = path.trim();
    config::save_compiler_path(
        &PathBuf::from(dir),
        if path.is_empty() { None } else { Some(path) },
    )
}

/// Returns whether `dir`'s project enables running PapyrusCompiler.exe as
/// part of linting a dropped `.psc` (see [`lint_psc_file`]/
/// [`compiler::check_psc_file`]), `false` by default.
#[tauri::command(async)]
pub(crate) fn load_compile_check(dir: String) -> Result<bool, String> {
    config::load_compile_check(&PathBuf::from(dir))
}

/// Persists whether `dir`'s project runs PapyrusCompiler.exe as part of
/// linting a dropped `.psc`.
#[tauri::command(async)]
pub(crate) fn save_compile_check(dir: String, enabled: bool) -> Result<(), String> {
    config::save_compile_check(&PathBuf::from(dir), enabled)
}

/// Returns `dir`'s configured additional script root directories (see
/// [`papyrus_lint_core::script_locator`]), if any. These are searched
/// alongside the conventional `scripts/source`/`source/scripts` directories
/// when resolving cross-script lookups (the "Argument type check"/"Return
/// type check" lints, autocompletion) and are appended to the compiler's
/// `-i` argument.
#[tauri::command(async)]
pub(crate) fn load_script_roots(dir: String) -> Result<Vec<String>, String> {
    config::load_script_roots(&PathBuf::from(dir))
}

/// Returns `dir`'s configured analysis-only lookup directories (see
/// [`papyrus_lint_core::function_table::FunctionTable::with_lookup_roots`]),
/// if any. These are searched only after conventional/`additional_script_roots`
/// directories, never linted, and never considered by
/// `conflicting_script_versions`.
#[tauri::command(async)]
pub(crate) fn load_lookup_script_roots(dir: String) -> Result<Vec<String>, String> {
    config::load_lookup_script_roots(&PathBuf::from(dir))
}

/// Reports the project paths discovered by the backend for display in the
/// Settings tab. Only script search directories that exist are included.
#[tauri::command(async)]
pub(crate) fn load_project_info(dir: String) -> Result<ProjectInfo, String> {
    let root = PathBuf::from(dir);
    let additional_roots = config::load_script_roots(&root)?;
    Ok(ProjectInfo {
        detected_script_roots: script_locator::detected_script_roots(&root, &additional_roots)
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        used_configuration_file: config::config_file_path(&root)
            .map(|path| path.to_string_lossy().into_owned()),
    })
}

/// Persists `roots` as `dir`'s configured additional script root
/// directories.
#[tauri::command(async)]
pub(crate) fn save_script_roots(dir: String, roots: Vec<String>) -> Result<(), String> {
    config::save_script_roots(&PathBuf::from(dir), &roots)
}

/// Persists `roots` as `dir`'s configured analysis-only lookup directories.
#[tauri::command(async)]
pub(crate) fn save_lookup_script_roots(dir: String, roots: Vec<String>) -> Result<(), String> {
    config::save_lookup_script_roots(&PathBuf::from(dir), &roots)
}

#[cfg(test)]
#[path = "lint_config_tests.rs"]
mod tests;
