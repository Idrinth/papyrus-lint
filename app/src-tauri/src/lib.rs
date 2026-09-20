mod config_presets;
mod export;
mod files;
mod lint;
mod lint_config;
mod meta;
mod project_root;
mod repair;

use config_presets::*;
use export::*;
use files::*;
use lint::*;
use lint_config::*;
use meta::*;
use project_root::*;
use repair::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            list_rule_tags,
            parse_achlist_file,
            parse_ppj_file,
            list_psc_files_recursively,
            parse_papyrus_script,
            lint_papyrus_script,
            parse_psc_file,
            read_psc_file,
            hash_psc_file_md5,
            get_psc_file_mtimes,
            write_psc_file,
            load_lint_config,
            save_lint_config,
            load_lint_config_from_path,
            save_lint_config_to_path,
            load_compiler_path,
            save_compiler_path,
            load_compile_check,
            save_compile_check,
            load_script_roots,
            load_lookup_script_roots,
            load_project_info,
            save_script_roots,
            save_lookup_script_roots,
            list_config_presets,
            apply_config_preset,
            get_preset_lint_config,
            save_config_as_preset,
            rename_user_preset,
            delete_user_preset,
            export_user_preset,
            lint_psc_file,
            preload_project_scripts,
            repair_psc_file,
            preview_repair_psc_file,
            preview_repair_psc_line,
            repair_psc_finding,
            repair_psc_file_rule,
            add_disable_comment_to_psc_line,
            add_disable_file_comment_to_psc_line,
            compile_psc_file,
            list_script_members,
            find_project_root,
            find_psc_project_root_for_path,
            format_issues_as_text,
            format_issues_as_json,
            format_issues_for_ai_base
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
