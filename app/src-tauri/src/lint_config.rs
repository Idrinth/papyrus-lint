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
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn config_commands_round_trip_lint_and_compiler_settings() {
        let dir = tempdir().unwrap();
        let dir_string = dir.path().to_string_lossy().into_owned();
        let config = papyrus_lints::Config {
            semicolon: true,
            indentation_width: 2,
            ..papyrus_lints::Config::default()
        };

        assert_eq!(
            load_lint_config(dir_string.clone()).unwrap(),
            Default::default()
        );
        save_lint_config(dir_string.clone(), config.clone()).unwrap();
        assert_eq!(load_lint_config(dir_string.clone()).unwrap(), config);

        save_compiler_path(dir_string.clone(), "  /tools/compiler  ".to_string()).unwrap();
        assert_eq!(
            load_compiler_path(dir_string.clone()).unwrap(),
            Some("/tools/compiler".to_string())
        );
        save_compiler_path(dir_string.clone(), " \t ".to_string()).unwrap();
        assert_eq!(load_compiler_path(dir_string.clone()).unwrap(), None);

        assert!(!load_compile_check(dir_string.clone()).unwrap());
        save_compile_check(dir_string.clone(), true).unwrap();
        assert!(load_compile_check(dir_string.clone()).unwrap());
        save_compile_check(dir_string.clone(), false).unwrap();
        assert!(!load_compile_check(dir_string.clone()).unwrap());

        assert_eq!(
            load_script_roots(dir_string.clone()).unwrap(),
            Vec::<String>::new()
        );
        save_script_roots(dir_string.clone(), vec!["../SharedScripts".to_string()]).unwrap();
        assert_eq!(
            load_script_roots(dir_string).unwrap(),
            vec!["../SharedScripts".to_string()]
        );
    }

    #[test]
    fn lint_config_from_path_commands_round_trip_regardless_of_project_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("custom-config.yaml");
        let path_string = path.to_string_lossy().into_owned();
        let config = papyrus_lints::Config {
            semicolon: true,
            indentation_width: 2,
            ..papyrus_lints::Config::default()
        };

        assert!(load_lint_config_from_path(path_string.clone()).is_err());
        save_lint_config_to_path(path_string.clone(), config.clone()).unwrap();
        assert_eq!(load_lint_config_from_path(path_string).unwrap(), config);
    }

    #[test]
    fn lint_config_from_path_commands_report_parse_and_write_errors() {
        let dir = tempdir().unwrap();
        let invalid = dir.path().join("invalid.yaml");
        std::fs::write(&invalid, "semicolon: [").unwrap();

        assert!(load_lint_config_from_path(invalid.to_string_lossy().into_owned()).is_err());
        assert!(save_lint_config_to_path(
            dir.path()
                .join("missing/config.yaml")
                .to_string_lossy()
                .into_owned(),
            Default::default(),
        )
        .is_err());
    }

    #[test]
    fn project_info_reports_detected_roots_and_configuration_file() {
        let dir = tempdir().unwrap();
        let scripts = dir.path().join("scripts/source");
        let shared = dir.path().join("shared");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(
            dir.path().join("papyrus-lint.yml"),
            "additional_script_roots:\n  - shared\n",
        )
        .unwrap();

        let info = load_project_info(dir.path().to_string_lossy().into_owned()).unwrap();
        assert_eq!(
            info,
            ProjectInfo {
                detected_script_roots: vec![
                    scripts.to_string_lossy().into_owned(),
                    shared.to_string_lossy().into_owned(),
                ],
                used_configuration_file: Some(
                    dir.path()
                        .join("papyrus-lint.yml")
                        .to_string_lossy()
                        .into_owned()
                ),
            }
        );
    }

    #[test]
    fn project_info_is_empty_when_the_project_has_no_known_layout_or_config() {
        let dir = tempdir().unwrap();

        let info = load_project_info(dir.path().to_string_lossy().into_owned()).unwrap();

        assert_eq!(
            info,
            ProjectInfo {
                detected_script_roots: Vec::new(),
                used_configuration_file: None,
            }
        );
    }

    #[test]
    fn project_info_reports_an_invalid_project_configuration() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("papyrus-lint.yaml"), "semicolon: [").unwrap();

        let error = load_project_info(dir.path().to_string_lossy().into_owned())
            .expect_err("invalid project configuration should be reported");

        assert!(error.contains("papyrus-lint.yaml"));
    }

    #[test]
    fn config_commands_report_invalid_yaml_and_unwritable_directories() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("papyrus-lint.yaml"), "semicolon: [").unwrap();
        let dir_string = dir.path().to_string_lossy().into_owned();

        assert!(load_lint_config(dir_string.clone()).is_err());
        assert!(load_compiler_path(dir_string.clone()).is_err());
        assert!(save_compiler_path(dir_string.clone(), "compiler".to_string()).is_err());
        assert!(load_compile_check(dir_string.clone()).is_err());
        assert!(save_compile_check(dir_string.clone(), true).is_err());
        assert!(load_script_roots(dir_string.clone()).is_err());
        assert!(
            save_script_roots(dir_string.clone(), vec!["../SharedScripts".to_string()]).is_err()
        );
        assert!(save_lint_config(dir_string, Default::default()).is_err());

        let missing_dir = dir.path().join("missing").to_string_lossy().into_owned();
        assert!(save_lint_config(missing_dir, Default::default()).is_err());
    }

    #[test]
    fn load_compiler_path_auto_detects_an_adjacent_compiler_executable() {
        let root = tempdir().unwrap();
        let compiler_dir = root.path().join("Papyrus Compiler");
        std::fs::create_dir(&compiler_dir).unwrap();
        let compiler = compiler_dir.join("PapyrusCompiler.exe");
        std::fs::write(&compiler, b"").unwrap();
        let data_dir = root.path().join("Data");
        std::fs::create_dir(&data_dir).unwrap();

        assert_eq!(
            load_compiler_path(data_dir.to_string_lossy().into_owned()).unwrap(),
            Some(compiler.to_string_lossy().into_owned())
        );
    }
}
