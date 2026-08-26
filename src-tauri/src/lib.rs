pub mod archlist;
pub mod config;
pub mod function_table;
pub mod script_locator;

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SemicolonStyle {
    Require,
    Forbid,
}

impl From<SemicolonStyle> for papyrus_lints::semicolon::Style {
    fn from(value: SemicolonStyle) -> Self {
        match value {
            SemicolonStyle::Require => Self::Require,
            SemicolonStyle::Forbid => Self::Forbid,
        }
    }
}

/// Parses the `.archlist` file at `path` and returns the resolved paths it lists.
#[tauri::command]
fn parse_archlist_file(path: String) -> Result<Vec<String>, String> {
    let entries = archlist::parse_archlist(&PathBuf::from(path)).map_err(|err| err.to_string())?;

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

/// Looks for a papyrus-lint YAML config file in `dir` (conventionally the
/// directory containing the `.archlist` file) and returns the lint
/// configuration it describes, falling back to the default configuration
/// if `dir` has no config file.
#[tauri::command]
fn load_lint_config(dir: String) -> Result<papyrus_lints::Config, String> {
    config::load_config(&PathBuf::from(dir))
}

/// Reads the `.psc` file at `path` and runs every lint rule against it,
/// honoring `config` and the configured `semicolon_style`. `root` is the
/// project root (conventionally the directory containing the `.archlist`
/// file); it lets the "Argument type check" lint resolve calls to
/// functions declared on other scripts under `root`.
#[tauri::command]
fn lint_psc_file(
    path: String,
    root: String,
    semicolon_style: SemicolonStyle,
    config: papyrus_lints::Config,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let source = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let mut function_table = function_table::FunctionTable::new(PathBuf::from(root));
    Ok(papyrus_lints::lint_with_semicolons_and_external_arguments(
        &source,
        semicolon_style.into(),
        &config,
        &mut function_table,
    ))
}

/// Reads the `.psc` file at `path`, applies every automatic fix (honoring
/// `config`, `semicolon_style`, and `indentation`), writes the repaired
/// source back to disk, and returns the diagnostics that remain. See
/// [`lint_psc_file`] for `root`.
#[tauri::command]
fn repair_psc_file(
    path: String,
    root: String,
    semicolon_style: SemicolonStyle,
    indentation: papyrus_lints::indentation::Indentation,
    config: papyrus_lints::Config,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let source = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let style = semicolon_style.into();
    let repaired = papyrus_lints::repair_with_semicolons(&source, style, indentation, &config);
    if repaired != source {
        std::fs::write(&path, &repaired).map_err(|err| err.to_string())?;
    }
    let mut function_table = function_table::FunctionTable::new(PathBuf::from(root));
    Ok(papyrus_lints::lint_with_semicolons_and_external_arguments(
        &repaired,
        style,
        &config,
        &mut function_table,
    ))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            parse_archlist_file,
            parse_papyrus_script,
            lint_papyrus_script,
            parse_psc_file,
            load_lint_config,
            lint_psc_file,
            repair_psc_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
