use serde_json::json;

use super::{disable_rule_in_yaml, full_range, provide, replace_line};
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

#[test]
fn provided_diagnostics_are_filtered_and_compiler_errors_cannot_be_ignored() {
    let (_dir, documents, uri) = open_script("Scriptname Quest\n");
    let actions = provide(
        &documents,
        &json!({
            "textDocument": { "uri": uri },
            "range": { "start": { "line": 0 }, "end": { "line": 0 } },
            "context": { "diagnostics": [
                { "source": "another-linter", "range": { "start": { "line": 0 } } },
                { "source": "papyrus-lint", "code": "compiler-error", "range": { "start": { "line": 0 } } }
            ] }
        }),
    );
    assert_eq!(actions, json!([]));
}

#[test]
fn quickfix_subkinds_are_accepted_but_missing_documents_are_empty() {
    let documents = Documents::default();
    let actions = provide(
        &documents,
        &json!({
            "textDocument": { "uri": "file:///missing.psc" },
            "context": { "only": ["quickfix.rewrite"] }
        }),
    );
    assert_eq!(actions, json!([]));
    assert_eq!(provide(&documents, &json!({})), json!([]));
}

#[test]
fn project_ignore_updates_an_existing_config() {
    let (dir, documents, uri) = open_script("Scriptname Quest \n");
    let config = dir.path().join("papyrus-lint.yml");
    std::fs::write(&config, "rules:\n  trailing_whitespace: true # keep note\n").unwrap();
    let diagnostic = json!({
        "source": "papyrus-lint",
        "code": "trailing-whitespace",
        "range": { "start": { "line": 0, "character": 16 }, "end": { "line": 0, "character": 17 } }
    });
    let actions = provide(
        &documents,
        &json!({
            "textDocument": { "uri": uri },
            "range": { "start": { "line": 0 }, "end": { "line": 0 } },
            "context": { "diagnostics": [diagnostic] }
        }),
    );
    let project = actions
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["title"].as_str().unwrap().contains("project"))
        .unwrap();
    let change = &project["edit"]["documentChanges"][0];
    assert!(change.get("kind").is_none());
    assert!(change["textDocument"]["uri"]
        .as_str()
        .unwrap()
        .ends_with("papyrus-lint.yml"));
    assert_eq!(
        change["edits"][0]["newText"],
        "rules:\n  trailing_whitespace: false # keep note\n"
    );
}

#[test]
fn yaml_updates_preserve_layout_and_avoid_duplicate_disables() {
    assert_eq!(
        disable_rule_in_yaml("anything: true", "x"),
        "anything: true\nrules:\n  x: false\n"
    );
    assert_eq!(
        disable_rule_in_yaml("rules:\r\n\tother: true\r\n", "x"),
        "rules:\r\n\tother: true\r\n\tx: false\r\n"
    );
    let disabled = "rules:\n  trailing_whitespace: false\n";
    assert_eq!(
        disable_rule_in_yaml(disabled, "trailing-whitespace"),
        disabled
    );
    assert_eq!(disable_rule_in_yaml("rules:\n", ""), "rules:\n");
}

#[test]
fn replacement_helpers_handle_utf16_crlf_and_invalid_lines() {
    assert_eq!(
        full_range("first\r\n😀"),
        json!({ "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 2 } })
    );
    assert_eq!(
        replace_line("a\r\nb\r\n", 2, "c\n").as_deref(),
        Some("a\r\nc\r\n")
    );
    assert!(replace_line("a", 0, "b").is_none());
    assert!(replace_line("a", 2, "b").is_none());
}
