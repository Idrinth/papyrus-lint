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
