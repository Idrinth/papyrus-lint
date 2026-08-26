pub mod archlist;
pub mod script_locator;

use std::path::PathBuf;

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

/// Reads the `.psc` file at `path` and parses it into a `Script` AST.
#[tauri::command]
fn parse_psc_file(path: String) -> Result<papyrus_parser::ast::Script, String> {
    let source = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    papyrus_parser::parse(&source).map_err(|err| err.to_string())
}

/// Parses Papyrus source and runs the forbidden-function lint against it,
/// returning any findings.
#[tauri::command]
fn lint_papyrus_script(
    source: &str,
) -> Result<Vec<papyrus_parser::lints::forbidden_functions::Finding>, String> {
    let script = papyrus_parser::parse(source).map_err(|e| e.to_string())?;
    Ok(papyrus_parser::lints::forbidden_functions::lint_forbidden_functions(&script))
}

/// Reads the `.psc` file at `path`, parses it, and runs the
/// forbidden-function lint against it, returning any findings.
#[tauri::command]
fn lint_psc_file(
    path: String,
) -> Result<Vec<papyrus_parser::lints::forbidden_functions::Finding>, String> {
    let source = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let script = papyrus_parser::parse(&source).map_err(|err| err.to_string())?;
    Ok(papyrus_parser::lints::forbidden_functions::lint_forbidden_functions(&script))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            parse_archlist_file,
            parse_papyrus_script,
            parse_psc_file,
            lint_papyrus_script,
            lint_psc_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
