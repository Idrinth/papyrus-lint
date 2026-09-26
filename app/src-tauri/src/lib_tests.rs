use super::*;
use tauri::test::{assert_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};

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
        tauri::webview::InvokeRequest {
            cmd: "get_app_version".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
        Ok(env!("CARGO_PKG_VERSION")),
    );
}
