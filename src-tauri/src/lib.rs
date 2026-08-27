pub mod achlist;
pub mod config;
pub mod function_table;
pub mod script_locator;

use std::path::PathBuf;

/// Parses the `.achlist` file at `path` and returns the resolved paths it lists.
#[tauri::command]
fn parse_achlist_file(path: String) -> Result<Vec<String>, String> {
    let entries = achlist::parse_achlist(&PathBuf::from(path)).map_err(|err| err.to_string())?;

    Ok(entries
        .into_iter()
        .map(|entry| entry.to_string_lossy().into_owned())
        .collect())
}

#[tauri::command]
fn parse_papyrus_script(source: &str) -> Result<papyrus_parser::ast::Script, String> {
    papyrus_parser::parse(source).map_err(|e| e.to_string())
}

#[tauri::command]
fn lint_papyrus_script(
    source: &str,
    config: papyrus_lints::Config,
) -> Vec<papyrus_lints::Diagnostic> {
    papyrus_lints::lint(source, &config)
}

/// Reads the `.psc` file at `path` and parses it into a `Script` AST.
#[tauri::command]
fn parse_psc_file(path: String) -> Result<papyrus_parser::ast::Script, String> {
    let source = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    papyrus_parser::parse(&source).map_err(|err| err.to_string())
}

/// Reads the `.psc` file at `path` and returns its raw source text, for the
/// frontend's syntax-highlighted code viewer.
#[tauri::command]
fn read_psc_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|err| err.to_string())
}

/// Writes `contents` to the `.psc` file at `path`, replacing it on disk.
/// Used by the frontend's code viewer to persist edits made in its edit
/// mode.
#[tauri::command]
fn write_psc_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|err| err.to_string())
}

/// Looks for a papyrus-lint YAML config file in `dir` (conventionally the
/// directory containing the `.achlist` file) and returns the lint
/// configuration it describes, falling back to the default configuration
/// if `dir` has no config file.
#[tauri::command]
fn load_lint_config(dir: String) -> Result<papyrus_lints::Config, String> {
    config::load_config(&PathBuf::from(dir))
}

/// Writes `config` to `dir`'s papyrus-lint YAML config file (creating it,
/// as `papyrus-lint.yaml`, if `dir` has none yet), so the formatting
/// selected in the UI is remembered for next time.
#[tauri::command]
fn save_lint_config(dir: String, config: papyrus_lints::Config) -> Result<(), String> {
    config::save_config(&PathBuf::from(dir), &config)
}

/// Returns the PapyrusCompile.exe path to use for `dir`'s project: an
/// explicit override saved to its papyrus-lint config file, or, absent
/// one, a path auto-detected at `../Papyrus Compiler/PapyrusCompile.exe`
/// relative to `dir` (the directory containing the `.achlist` file).
/// Returns `null` if neither is available.
#[tauri::command]
fn load_compiler_path(dir: String) -> Result<Option<String>, String> {
    config::resolve_compiler_path(&PathBuf::from(dir))
}

/// Persists an explicit PapyrusCompile.exe path override to `dir`'s
/// papyrus-lint config file. Passing an empty (or blank) string clears
/// the override, reverting to auto-detection.
#[tauri::command]
fn save_compiler_path(dir: String, path: String) -> Result<(), String> {
    let path = path.trim();
    config::save_compiler_path(
        &PathBuf::from(dir),
        if path.is_empty() { None } else { Some(path) },
    )
}

/// Reads the `.psc` file at `path` and runs every lint rule against it,
/// honoring the semicolon style `config` selects. `root` is the project
/// root (conventionally the directory containing the `.achlist` file); it
/// lets the "Argument type check" lint resolve calls to functions declared
/// on other scripts under `root`.
#[tauri::command]
fn lint_psc_file(
    path: String,
    root: String,
    config: papyrus_lints::Config,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let source = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let mut function_table = function_table::FunctionTable::new(PathBuf::from(root));
    Ok(papyrus_lints::lint_with_external_arguments(
        &source,
        &config,
        &mut function_table,
    ))
}

/// Reads the `.psc` file at `path`, applies every automatic fix (honoring
/// the semicolon and indentation style `config` selects), writes the
/// repaired source back to disk, and returns the diagnostics that remain.
/// See [`lint_psc_file`] for `root`.
#[tauri::command]
fn repair_psc_file(
    path: String,
    root: String,
    config: papyrus_lints::Config,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let source = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let repaired = papyrus_lints::repair(&source, &config);
    if repaired != source {
        std::fs::write(&path, &repaired).map_err(|err| err.to_string())?;
    }
    let mut function_table = function_table::FunctionTable::new(PathBuf::from(root));
    Ok(papyrus_lints::lint_with_external_arguments(
        &repaired,
        &config,
        &mut function_table,
    ))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            parse_achlist_file,
            parse_papyrus_script,
            lint_papyrus_script,
            parse_psc_file,
            read_psc_file,
            write_psc_file,
            load_lint_config,
            save_lint_config,
            load_compiler_path,
            save_compiler_path,
            lint_psc_file,
            repair_psc_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
