pub mod archlist;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![parse_archlist_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
