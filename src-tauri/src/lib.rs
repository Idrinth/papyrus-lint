// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn parse_papyrus_script(source: &str) -> Result<papyrus_parser::ast::Script, String> {
    papyrus_parser::parse(source).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, parse_papyrus_script])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
