use std::io::Cursor;

use serde_json::{json, Value};

use super::{serve, FIX_FILE_COMMAND};
use crate::framing::write_message;

fn exchange(messages: &[Value]) -> (i32, Vec<Value>) {
    let mut input = Vec::new();
    for message in messages {
        write_message(&mut input, message.to_string().as_bytes()).unwrap();
    }
    let mut output = Vec::new();
    let code = serve(Cursor::new(input), &mut output).unwrap();
    let mut responses = Vec::new();
    let mut cursor = Cursor::new(output);
    while let Some(body) = crate::framing::read_message(&mut cursor).unwrap() {
        responses.push(serde_json::from_slice(&body).unwrap());
    }
    (code, responses)
}

fn request(id: i64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

#[test]
fn initialize_advertises_the_stub_capabilities() {
    let (_code, responses) = exchange(&[request(1, "initialize", json!({}))]);
    assert_eq!(responses.len(), 1);
    let result = &responses[0]["result"];
    assert_eq!(result["serverInfo"]["name"], "papyrus-lint-lsp");
    assert_eq!(result["serverInfo"]["version"], "0.1.0");
    assert_eq!(result["capabilities"]["textDocumentSync"]["change"], 1);
    assert_eq!(result["capabilities"]["codeActionProvider"], true);
    assert_eq!(
        result["capabilities"]["executeCommandProvider"]["commands"][0],
        FIX_FILE_COMMAND
    );
}

#[test]
fn document_sync_publishes_diagnostics() {
    let (_code, responses) = exchange(&[
        request(1, "initialize", json!({})),
        json!({ "jsonrpc": "2.0", "method": "initialized" }),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": "file:///Quest.psc", "languageId": "papyrus", "version": 1, "text": "Scriptname Quest \n" } }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": { "textDocument": { "uri": "file:///Quest.psc", "version": 2 }, "contentChanges": [{ "text": "Scriptname Quest \n" }] }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": { "textDocument": { "uri": "file:///Quest.psc" }, "text": "Scriptname Quest \n" }
        }),
        json!({ "jsonrpc": "2.0", "method": "textDocument/didClose", "params": { "textDocument": { "uri": "file:///Quest.psc" } } }),
        json!({ "jsonrpc": "2.0", "method": "workspace/didChangeConfiguration", "params": {} }),
        json!({ "jsonrpc": "2.0", "method": "$/cancelRequest", "params": { "id": 1 } }),
    ]);
    let published: Vec<_> = responses
        .iter()
        .filter(|message| message["method"] == "textDocument/publishDiagnostics")
        .collect();
    assert_eq!(published.len(), 4);
    assert!(published[0]["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "trailing-whitespace"));
    assert_eq!(published[3]["params"]["diagnostics"], json!([]));
    assert!(responses.iter().any(|message| message["id"] == 1));
}

#[test]
fn code_action_and_execute_command_are_empty() {
    let (_code, responses) = exchange(&[
        request(1, "initialize", json!({})),
        request(2, "textDocument/codeAction", json!({})),
        request(
            3,
            "workspace/executeCommand",
            json!({ "command": FIX_FILE_COMMAND, "arguments": [] }),
        ),
    ]);
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(responses[1]["result"], json!([]));
    assert_eq!(responses[2]["id"], 3);
    assert!(responses[2]["result"].is_null());
}

#[test]
fn unknown_request_is_method_not_found() {
    let (_code, responses) = exchange(&[
        request(1, "initialize", json!({})),
        request(4, "textDocument/completion", json!({})),
    ]);
    assert_eq!(responses[1]["error"]["code"], -32601);
    assert_eq!(responses[1]["id"], 4);
}

#[test]
fn unknown_notification_is_ignored() {
    let (_code, responses) = exchange(&[json!({
        "jsonrpc": "2.0",
        "method": "workspace/didChangeWatchedFiles"
    })]);
    assert!(responses.is_empty());
}

#[test]
fn exit_without_shutdown_returns_one() {
    let (code, _) = exchange(&[json!({ "jsonrpc": "2.0", "method": "exit" })]);
    assert_eq!(code, 1);
}

#[test]
fn shutdown_then_exit_returns_zero() {
    let (code, responses) = exchange(&[
        request(1, "initialize", json!({})),
        request(5, "shutdown", Value::Null),
        json!({ "jsonrpc": "2.0", "method": "exit" }),
    ]);
    assert_eq!(code, 0);
    assert!(responses[1]["result"].is_null());
    assert_eq!(responses[1]["id"], 5);
}

#[test]
fn request_before_initialize_is_rejected() {
    let (_code, responses) = exchange(&[request(6, "textDocument/codeAction", json!({}))]);
    assert_eq!(responses[0]["error"]["code"], -32002);
    assert_eq!(responses[0]["id"], 6);
}

#[test]
fn request_after_shutdown_is_rejected() {
    let (_code, responses) = exchange(&[
        request(1, "initialize", json!({})),
        request(2, "shutdown", Value::Null),
        request(3, "textDocument/codeAction", json!({})),
    ]);
    assert_eq!(responses[2]["error"]["code"], -32600);
    assert_eq!(responses[2]["id"], 3);
}

#[test]
fn second_initialize_is_rejected() {
    let (_code, responses) = exchange(&[
        request(1, "initialize", json!({})),
        request(2, "initialize", json!({})),
    ]);
    assert_eq!(responses[1]["error"]["code"], -32600);
    assert_eq!(responses[1]["id"], 2);
}

#[test]
fn parse_error_and_non_object_are_reported() {
    let mut input = Vec::new();
    write_message(&mut input, b"not-json").unwrap();
    write_message(&mut input, b"[]").unwrap();
    let mut output = Vec::new();
    serve(Cursor::new(input), &mut output).unwrap();
    let mut cursor = Cursor::new(output);
    let first: Value =
        serde_json::from_slice(&crate::framing::read_message(&mut cursor).unwrap().unwrap())
            .unwrap();
    let second: Value =
        serde_json::from_slice(&crate::framing::read_message(&mut cursor).unwrap().unwrap())
            .unwrap();
    assert_eq!(first["error"]["code"], -32700);
    assert!(first["id"].is_null());
    assert_eq!(second["error"]["code"], -32600);
}

#[test]
fn request_without_a_method_is_invalid() {
    let (_code, responses) = exchange(&[json!({ "jsonrpc": "2.0", "id": 9 })]);
    assert_eq!(responses[0]["error"]["code"], -32600);
    assert_eq!(responses[0]["id"], 9);
}

#[test]
fn string_ids_are_echoed() {
    let (_code, responses) = exchange(&[json!({
        "jsonrpc": "2.0",
        "id": "abc",
        "method": "initialize",
        "params": {}
    })]);
    assert_eq!(responses[0]["id"], "abc");
}

#[test]
fn fix_file_applies_every_automatic_fix() {
    let uri = "file:///Quest.psc";
    let (_code, responses) = exchange(&[
        request(1, "initialize", json!({})),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": uri, "version": 1, "text": "Scriptname Quest \n" } }
        }),
        request(
            7,
            "workspace/executeCommand",
            json!({
                "command": FIX_FILE_COMMAND,
                "arguments": [uri]
            }),
        ),
        json!({
            "jsonrpc": "2.0",
            "id": "papyrus-lint-1",
            "result": { "applied": true }
        }),
    ]);
    let edit = responses
        .iter()
        .find(|message| message["method"] == "workspace/applyEdit")
        .unwrap();
    assert_eq!(
        edit["params"]["edit"]["documentChanges"][0]["textDocument"],
        json!({ "uri": uri, "version": 1 })
    );
    let new_text = edit["params"]["edit"]["documentChanges"][0]["edits"][0]["newText"]
        .as_str()
        .unwrap();
    assert_eq!(new_text, "Scriptname Quest\n");
    let command_result = responses.iter().find(|message| message["id"] == 7).unwrap();
    assert!(command_result["result"].is_null());
    let published: Vec<_> = responses
        .iter()
        .filter(|message| message["method"] == "textDocument/publishDiagnostics")
        .collect();
    let last = published.last().unwrap();
    assert!(last["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .all(|diagnostic| diagnostic["code"] != "trailing-whitespace"));
}

#[test]
fn fix_file_preserves_a_change_received_while_applying_the_edit() {
    let uri = "file:///Quest.psc";
    let changed = "Scriptname Quest\n; typing after the fix was requested\n";
    let (_code, responses) = exchange(&[
        request(1, "initialize", json!({})),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": uri, "version": 1, "text": "Scriptname Quest \n" } }
        }),
        request(
            7,
            "workspace/executeCommand",
            json!({ "command": FIX_FILE_COMMAND, "arguments": [uri] }),
        ),
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri, "version": 2 },
                "contentChanges": [{ "text": changed }]
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": "papyrus-lint-1",
            "result": { "applied": true }
        }),
        request(
            8,
            "textDocument/codeAction",
            json!({
                "textDocument": { "uri": uri },
                "range": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 1, "character": 0 }
                },
                "context": { "diagnostics": [] }
            }),
        ),
    ]);

    let edit = responses
        .iter()
        .find(|message| message["method"] == "workspace/applyEdit")
        .unwrap();
    assert_eq!(
        edit["params"]["edit"]["documentChanges"][0]["textDocument"]["version"],
        1
    );
    let published: Vec<_> = responses
        .iter()
        .filter(|message| message["method"] == "textDocument/publishDiagnostics")
        .collect();
    assert_eq!(published.len(), 2);
    assert_eq!(published.last().unwrap()["params"]["version"], 2);
    assert_eq!(responses.last().unwrap()["id"], 8);
}
