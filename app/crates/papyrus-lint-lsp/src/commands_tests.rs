use serde_json::json;

use super::{command_uri, repaired_text};

#[test]
fn reads_a_uri_from_the_shapes_editors_send() {
    assert_eq!(
        command_uri(&json!({ "arguments": ["file:///Quest.psc"] })).as_deref(),
        Some("file:///Quest.psc")
    );
    assert_eq!(
        command_uri(&json!({ "arguments": [{ "uri": "file:///Quest.psc" }] })).as_deref(),
        Some("file:///Quest.psc")
    );
    assert_eq!(
        command_uri(&json!({ "arguments": [{ "textDocument": { "uri": "file:///A.psc" } }] }))
            .as_deref(),
        Some("file:///A.psc")
    );
    assert!(command_uri(&json!({ "arguments": [] })).is_none());
}

#[test]
fn repair_strips_trailing_whitespace() {
    let repaired = repaired_text("Scriptname Quest \n", "file:///Quest.psc").unwrap();
    assert_eq!(repaired, "Scriptname Quest\n");
    assert!(repaired_text("Scriptname Quest\n", "file:///Quest.psc").is_none());
}
