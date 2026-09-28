use super::*;
use std::fs;
use tauri::test::{assert_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tempfile::tempdir;

fn invoke_request(command: &str, body: serde_json::Value) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: command.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: "tauri://localhost".parse().unwrap(),
        body: tauri::ipc::InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

#[test]
fn project_root_uses_the_first_entry_in_a_supported_script_tree() {
    let dir = tempdir().unwrap();
    let first_root = dir.path().join("first-project");
    let second_root = dir.path().join("second-project");
    let first_script = first_root.join("scripts/source/nested/First.psc");
    let second_script = second_root.join("source/scripts/Second.psc");

    assert_eq!(
        find_project_root(
            vec![
                dir.path()
                    .join("unmatched/Example.psc")
                    .to_string_lossy()
                    .into_owned(),
                first_script.to_string_lossy().into_owned(),
                second_script.to_string_lossy().into_owned(),
            ],
            "fallback".to_string(),
        ),
        first_root.to_string_lossy()
    );
}

#[test]
fn project_root_returns_the_fallback_when_no_entry_matches() {
    assert_eq!(
        find_project_root(
            vec!["custom/source/Example.psc".to_string()],
            "selected/project".to_string(),
        ),
        "selected/project"
    );
    assert_eq!(
        find_project_root(Vec::new(), "empty/project".to_string()),
        "empty/project"
    );
}

#[test]
fn project_root_requires_adjacent_supported_directory_names() {
    let separator = std::path::MAIN_SEPARATOR;

    assert_eq!(
        find_project_root(
            vec![
                format!(
                    "project{separator}scripts{separator}generated{separator}source{separator}Example.psc"
                ),
                format!(
                    "project{separator}source-files{separator}scripts{separator}Example.psc"
                ),
            ],
            "fallback".to_string(),
        ),
        "fallback"
    );
}

#[test]
fn project_root_recognizes_the_source_scripts_directory_order_for_files() {
    let separator = std::path::MAIN_SEPARATOR;

    assert_eq!(
        find_project_root(
            vec![format!(
                "project{separator}source{separator}scripts{separator}nested{separator}Example.psc"
            )],
            "fallback".to_string(),
        ),
        "project"
    );
}

#[test]
fn psc_project_root_command_handles_conventional_and_fallback_layouts() {
    let separator = std::path::MAIN_SEPARATOR;

    assert_eq!(
        find_psc_project_root_for_path(format!(
            "project{separator}Scripts{separator}Source{separator}nested{separator}Example.psc"
        )),
        "project"
    );
    assert_eq!(
        find_psc_project_root_for_path(format!(
            "project{separator}custom{separator}source{separator}Example.psc"
        )),
        "project"
    );
    assert_eq!(
        find_psc_project_root_for_path("Example.psc".to_string()),
        "."
    );
}

#[test]
fn psc_project_root_command_prefers_a_configured_ancestor_over_the_directory_pair() {
    let dir = tempdir().unwrap();
    let configured_root = dir.path().join("configured-project");
    let conventional_root = configured_root.join("Data");
    let script = conventional_root.join("Scripts/Source/Nested/Example.psc");

    fs::create_dir_all(script.parent().unwrap()).unwrap();
    fs::write(configured_root.join("papyrus-lint.yaml"), "rules: {}\n").unwrap();
    fs::write(&script, "ScriptName Example\n").unwrap();

    assert_eq!(
        find_psc_project_root_for_path(script.to_string_lossy().into_owned()),
        configured_root.to_string_lossy()
    );
}

#[test]
fn psc_project_root_command_recognizes_yml_configs_in_custom_layouts() {
    let dir = tempdir().unwrap();
    let configured_root = dir.path().join("configured-project");
    let script = configured_root.join("custom/deep/layout/Example.psc");

    fs::create_dir_all(script.parent().unwrap()).unwrap();
    fs::write(configured_root.join("papyrus-lint.yml"), "rules: {}\n").unwrap();
    fs::write(&script, "ScriptName Example\n").unwrap();

    assert_eq!(
        find_psc_project_root_for_path(script.to_string_lossy().into_owned()),
        configured_root.to_string_lossy()
    );
}

#[test]
fn psc_project_root_command_uses_the_nearest_configured_ancestor() {
    let dir = tempdir().unwrap();
    let outer_root = dir.path().join("workspace");
    let project_root = outer_root.join("project");
    let script = project_root.join("custom/deep/layout/Example.psc");

    fs::create_dir_all(script.parent().unwrap()).unwrap();
    fs::write(outer_root.join("papyrus-lint.yaml"), "rules: {}\n").unwrap();
    fs::write(project_root.join("papyrus-lint.yml"), "rules: {}\n").unwrap();

    assert_eq!(
        find_psc_project_root_for_path(script.to_string_lossy().into_owned()),
        project_root.to_string_lossy()
    );
}

#[test]
fn project_root_accepts_mixed_case_directory_pairs_and_directory_entries() {
    let separator = std::path::MAIN_SEPARATOR;

    assert_eq!(
        find_project_root(
            vec![format!(
                "project{separator}SoUrCe{separator}ScRiPtS{separator}Nested"
            )],
            "fallback".to_string(),
        ),
        "project"
    );
}

#[test]
fn project_root_commands_accept_frontend_arguments_over_ipc() {
    let app = crate::configure_builder(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    let webview = tauri::WebviewWindowBuilder::new(&app, "project-root-test", Default::default())
        .build()
        .unwrap();
    let separator = std::path::MAIN_SEPARATOR;

    assert_ipc_response(
        &webview,
        invoke_request(
            "find_project_root",
            serde_json::json!({
                "entries": [format!(
                    "project{separator}scripts{separator}source{separator}Example.psc"
                )],
                "fallback": "fallback"
            }),
        ),
        Ok("project"),
    );
    assert_ipc_response(
        &webview,
        invoke_request(
            "find_project_root",
            serde_json::json!({
                "entries": [],
                "fallback": "selected/project"
            }),
        ),
        Ok("selected/project"),
    );
    assert_ipc_response(
        &webview,
        invoke_request(
            "find_psc_project_root_for_path",
            serde_json::json!({
                "path": format!(
                    "project{separator}custom{separator}source{separator}Example.psc"
                )
            }),
        ),
        Ok("project"),
    );
}
