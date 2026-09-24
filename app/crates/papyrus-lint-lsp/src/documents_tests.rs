use std::io::Cursor;

use serde_json::json;

use super::{config_for_uri, Documents};
use crate::framing::read_message;

fn published(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut documents = Documents::default();
    let mut output = Vec::new();
    for message in messages {
        let method = message["method"].as_str().unwrap();
        let params = &message["params"];
        match method {
            "textDocument/didOpen" => documents.did_open(params, &mut output).unwrap(),
            "textDocument/didChange" => documents.did_change(params, &mut output).unwrap(),
            "textDocument/didSave" => documents.did_save(params, &mut output).unwrap(),
            "textDocument/didClose" => documents.did_close(params, &mut output).unwrap(),
            other => panic!("unexpected {other}"),
        }
    }
    let mut cursor = Cursor::new(output);
    let mut notifications = Vec::new();
    while let Some(body) = read_message(&mut cursor).unwrap() {
        notifications.push(serde_json::from_slice(&body).unwrap());
    }
    notifications
}

#[test]
fn open_publishes_trailing_whitespace() {
    let notifications = published(&[json!({
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///Quest.psc",
                "languageId": "papyrus",
                "version": 3,
                "text": "Scriptname Quest \n"
            }
        }
    })]);
    assert_eq!(notifications.len(), 1);
    let params = &notifications[0]["params"];
    assert_eq!(
        notifications[0]["method"],
        "textDocument/publishDiagnostics"
    );
    assert!(notifications[0].get("id").is_none());
    assert_eq!(params["uri"], "file:///Quest.psc");
    assert_eq!(params["version"], 3);
    let diagnostic = &params["diagnostics"][0];
    assert_eq!(diagnostic["code"], "trailing-whitespace");
    assert_eq!(diagnostic["severity"], 2);
    assert_eq!(diagnostic["range"]["start"]["line"], 0);
}

#[test]
fn change_and_close_clear_a_fixed_script() {
    let notifications = published(&[
        json!({
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": "file:///Quest.psc", "version": 1, "text": "Scriptname Quest \n" } }
        }),
        json!({
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": "file:///Quest.psc", "version": 2 },
                "contentChanges": [{ "text": "Scriptname Quest\n" }]
            }
        }),
        json!({
            "method": "textDocument/didClose",
            "params": { "textDocument": { "uri": "file:///Quest.psc" } }
        }),
    ]);
    let last_change = &notifications[1]["params"]["diagnostics"];
    assert!(last_change
        .as_array()
        .unwrap()
        .iter()
        .all(|diagnostic| diagnostic["code"] != "trailing-whitespace"));
    assert_eq!(notifications[2]["params"]["diagnostics"], json!([]));
}

#[test]
fn project_config_turns_the_rule_off() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("papyrus-lint.yaml");
    std::fs::write(&config, "rules:\n  trailing_whitespace: false\n").unwrap();
    let script = dir.path().join("Quest.psc");
    let uri = format!("file://{}", script.display());
    let loaded = config_for_uri(&uri);
    assert!(!loaded.rules.trailing_whitespace);

    let notifications = published(&[json!({
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": uri,
                "version": 1,
                "text": "Scriptname Quest \n"
            }
        }
    })]);
    let diagnostics = notifications[0]["params"]["diagnostics"]
        .as_array()
        .unwrap();
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic["code"] != "trailing-whitespace"));
}
