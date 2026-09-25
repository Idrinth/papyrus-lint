use serde_json::json;

use super::{disable_rule_in_yaml, provide};
use crate::documents::Documents;

fn open_script(text: &str) -> (tempfile::TempDir, Documents, String) {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("Quest.psc");
    let uri = format!("file://{}", script.display());
    let mut documents = Documents::default();
    let mut output = Vec::new();
    documents
        .did_open(
            &json!({
                "textDocument": { "uri": uri, "version": 1, "text": text }
            }),
            &mut output,
        )
        .unwrap();
    (dir, documents, uri)
}

#[test]
fn offers_fix_and_ignore_for_trailing_whitespace() {
    let (_dir, documents, uri) = open_script("Scriptname Quest \n");
    let actions = provide(
        &documents,
        &json!({
            "textDocument": { "uri": uri },
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 1 } },
            "context": { "diagnostics": [] }
        }),
    );
    let titles: Vec<_> = actions
        .as_array()
        .unwrap()
        .iter()
        .map(|action| action["title"].as_str().unwrap())
        .collect();
    assert!(titles
        .iter()
        .any(|title| title.starts_with("Fix this issue")));
    assert!(titles
        .iter()
        .any(|title| title.starts_with("Ignore this lint for the line")));
    assert!(titles
        .iter()
        .any(|title| title.starts_with("Ignore this lint for the file")));
    assert!(titles
        .iter()
        .any(|title| title.starts_with("Ignore this lint for the project")));
    let fix = actions
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["title"].as_str().unwrap().starts_with("Fix"))
        .unwrap();
    assert_eq!(
        fix["edit"]["documentChanges"][0]["textDocument"],
        json!({ "uri": uri, "version": 1 })
    );
    let new_text = fix["edit"]["documentChanges"][0]["edits"][0]["newText"]
        .as_str()
        .unwrap();
    assert!(new_text.starts_with("Scriptname Quest\n") || new_text == "Scriptname Quest\n");
    assert!(!new_text.contains("Quest \n"));
}

#[test]
fn only_source_kind_filters_quickfixes_out() {
    let (_dir, documents, uri) = open_script("Scriptname Quest \n");
    let actions = provide(
        &documents,
        &json!({
            "textDocument": { "uri": uri },
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
            "context": { "only": ["source"], "diagnostics": [] }
        }),
    );
    assert_eq!(actions, json!([]));
}

#[test]
fn disable_rule_inserts_and_updates_a_rules_block() {
    let created = disable_rule_in_yaml("", "trailing-whitespace");
    assert_eq!(created, "rules:\n  trailing_whitespace: false\n");
    let updated = disable_rule_in_yaml(
        "rules:\n  trailing_whitespace: true\n",
        "trailing-whitespace",
    );
    assert!(updated.contains("trailing_whitespace: false"));
    assert!(!updated.contains("true"));
}
