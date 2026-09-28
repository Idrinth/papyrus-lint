use super::*;
use tauri::test::{
    assert_ipc_response, get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY,
};

fn invoke_request(command: &str) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: command.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: "tauri://localhost".parse().unwrap(),
        body: tauri::ipc::InvokeBody::default(),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

#[test]
fn configured_builder_registers_desktop_commands() {
    let app = configure_builder(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    let webview = tauri::WebviewWindowBuilder::new(&app, "test", Default::default())
        .build()
        .unwrap();

    assert_ipc_response(
        &webview,
        invoke_request("get_app_version"),
        Ok(env!("CARGO_PKG_VERSION")),
    );

    let rule_tags = get_ipc_response(&webview, invoke_request("list_rule_tags")).unwrap();
    let rule_tags = rule_tags.deserialize::<serde_json::Value>().unwrap();
    let rule_tags = rule_tags.as_array().unwrap();
    assert_eq!(rule_tags.len(), papyrus_lints::tags::RULE_TAGS.len());
    assert!(rule_tags.iter().all(|tags| {
        tags.get("rule").is_some()
            && tags.get("description").is_some()
            && tags.get("kinds").is_some()
            && tags.get("importance").is_some()
            && tags.get("auto_fixable").is_some()
            && tags.get("doc_url").is_some()
    }));
}

#[test]
fn configured_builder_registers_every_desktop_command() {
    let app = configure_builder(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    let webview = tauri::WebviewWindowBuilder::new(&app, "test", Default::default())
        .build()
        .unwrap();

    let commands = [
        "get_app_version",
        "list_rule_tags",
        "parse_achlist_file",
        "parse_ppj_file",
        "list_psc_files_recursively",
        "parse_papyrus_script",
        "lint_papyrus_script",
        "parse_psc_file",
        "read_psc_file",
        "hash_psc_file_md5",
        "get_psc_file_mtimes",
        "write_psc_file",
        "load_lint_config",
        "save_lint_config",
        "load_lint_config_from_path",
        "save_lint_config_to_path",
        "load_compiler_path",
        "save_compiler_path",
        "load_compile_check",
        "load_strict_achlist_scope",
        "save_compile_check",
        "load_script_roots",
        "load_lookup_script_roots",
        "load_project_info",
        "save_script_roots",
        "save_lookup_script_roots",
        "list_config_presets",
        "apply_config_preset",
        "get_preset_lint_config",
        "save_config_as_preset",
        "rename_user_preset",
        "delete_user_preset",
        "export_user_preset",
        "lint_psc_file",
        "preload_project_scripts",
        "lint_project_scripts",
        "repair_psc_file",
        "preview_repair_psc_file",
        "preview_repair_psc_line",
        "repair_psc_finding",
        "repair_psc_file_rule",
        "add_disable_comment_to_psc_line",
        "add_disable_file_comment_to_psc_line",
        "add_nodiscard_comment_to_psc_line",
        "compile_psc_file",
        "resolve_completion_query",
        "list_script_members",
        "find_project_root",
        "find_psc_project_root_for_path",
        "format_issues_as_text",
        "format_issues_as_json",
        "format_issues_for_ai_base",
    ];

    for command in commands {
        if let Err(error) = get_ipc_response(&webview, invoke_request(command)) {
            assert_ne!(
                error,
                format!("Command {command} not found"),
                "{command} was not registered"
            );
        }
    }
}

#[test]
fn configured_builder_rejects_unregistered_commands() {
    let app = configure_builder(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    let webview = tauri::WebviewWindowBuilder::new(&app, "test", Default::default())
        .build()
        .unwrap();

    assert_ipc_response(
        &webview,
        invoke_request("not_a_desktop_command"),
        Err("Command not_a_desktop_command not found"),
    );
}
