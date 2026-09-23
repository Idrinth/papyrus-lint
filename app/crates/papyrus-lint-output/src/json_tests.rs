use super::*;
use crate::diagnostic::OwnedDiagnostic;

fn owned(rule: &str, message: &str) -> OwnedDiagnostic {
    OwnedDiagnostic {
        line: 1,
        column: 2,
        rule: rule.to_string(),
        message: message.to_string(),
    }
}

#[test]
fn to_json_diagnostics_carries_line_column_rule_level_and_message() {
    let diagnostics = [owned(
        "trailing-whitespace",
        "[warning] trailing whitespace",
    )];

    let json = to_json_diagnostics(&diagnostics, false);

    assert_eq!(json.len(), 1);
    assert_eq!(json[0].line, 1);
    assert_eq!(json[0].column, 2);
    assert_eq!(json[0].rule, "trailing-whitespace");
    assert_eq!(json[0].level, "warning");
    assert_eq!(json[0].message, "[warning] trailing whitespace");
}

#[test]
fn to_json_diagnostics_can_strip_the_severity_prefix_while_keeping_the_level_field() {
    let diagnostics = [owned(
        "trailing-whitespace",
        "[warning] trailing whitespace",
    )];

    let json = to_json_diagnostics(&diagnostics, true);

    assert_eq!(json[0].level, "warning");
    assert_eq!(json[0].message, "trailing whitespace");
}

#[test]
fn to_json_diagnostics_looks_up_a_known_rules_doc_url() {
    let diagnostics = [owned(
        "trailing-whitespace",
        "[warning] trailing whitespace",
    )];

    let json = to_json_diagnostics(&diagnostics, false);

    assert_eq!(
        json[0].doc_url.as_deref(),
        Some("https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace")
    );
}

#[test]
fn to_json_diagnostics_leaves_doc_url_none_for_an_unknown_rule() {
    let diagnostics = [owned("compiler-error", "[error] syntax error")];

    let json = to_json_diagnostics(&diagnostics, false);

    assert_eq!(json[0].doc_url, None);
}

#[test]
fn to_json_diagnostics_works_directly_against_native_papyrus_lints_diagnostics() {
    let diagnostics = [papyrus_lints::Diagnostic {
        line: 3,
        column: 4,
        rule: "trailing-whitespace",
        message: "[warning] trailing whitespace".to_string(),
    }];

    let json = to_json_diagnostics(&diagnostics, false);

    assert_eq!(json[0].rule, "trailing-whitespace");
    assert_eq!(json[0].line, 3);
}

#[test]
fn json_file_report_serializes_parser_errors_alongside_diagnostics() {
    let report = JsonFileReport {
        path: "Broken.psc".to_string(),
        diagnostics: Vec::new(),
        parser_errors: vec![JsonParserError {
            kind: ParserErrorKind::Parse,
            line: 2,
            column: 18,
            message: "expected ')', found Newline".to_string(),
        }],
        diff: None,
    };

    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["path"], "Broken.psc");
    assert_eq!(value["diagnostics"], serde_json::json!([]));
    assert_eq!(
        value["parser_errors"],
        serde_json::json!([{
            "kind": "parse",
            "line": 2,
            "column": 18,
            "message": "expected ')', found Newline"
        }])
    );
    assert!(value["diff"].is_null());
}
