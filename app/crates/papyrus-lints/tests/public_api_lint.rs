//! Black-box tests for the crate-level lint entry point: diagnostic
//! contract, YAML switches, and opt-in rules.

use papyrus_lints::{lint, repair, Config, Diagnostic};

#[test]
fn lint_reports_multiple_enabled_rules_through_the_public_api() {
    let source = "ScriptName Example  \n\nFunction Run(Int left,Int right)\nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "trailing-whitespace"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "comma-spacing"));
}

#[test]
fn diagnostic_serialization_exposes_the_structured_output_contract() {
    let diagnostic = Diagnostic {
        line: 7,
        column: 11,
        message: "[warning] Example finding".to_string(),
        rule: "example-rule",
    };

    assert_eq!(
        serde_json::to_value(diagnostic).unwrap(),
        serde_json::json!({
            "line": 7,
            "column": 11,
            "message": "[warning] Example finding",
            "rule": "example-rule",
        })
    );
}

#[test]
fn diagnostic_level_requires_an_exact_leading_severity_tag() {
    let diagnostic = |message: &str| Diagnostic {
        line: 1,
        column: 1,
        message: message.to_string(),
        rule: "example-rule",
    };

    assert_eq!(diagnostic("[info] details").level(), "info");
    assert_eq!(diagnostic(" [warning] details").level(), "error");
    assert_eq!(diagnostic("[WARNING] details").level(), "error");
    assert_eq!(diagnostic("[warning-ish] details").level(), "error");
}

#[test]
fn public_diagnostics_have_valid_locations_and_severity_tags() {
    let source = "ScriptName Example  \n\nFunction Run(Int left,Int right)\nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(!diagnostics.is_empty());
    for diagnostic in diagnostics {
        assert!(diagnostic.line > 0, "{} had a zero line", diagnostic.rule);
        assert!(
            diagnostic.column > 0,
            "{} had a zero column",
            diagnostic.rule
        );
        assert!(
            diagnostic
                .message
                .starts_with(&format!("[{}]", diagnostic.level())),
            "{} did not have a recognized severity tag: {}",
            diagnostic.rule,
            diagnostic.message
        );
    }
}

#[test]
fn raw_source_rules_still_report_when_the_script_does_not_parse() {
    let source = "ScriptName Example\n\nFunction Broken(\n    Call(1,2)  \n";

    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.line == 4 && diagnostic.rule == "comma-spacing"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.line == 4 && diagnostic.rule == "trailing-whitespace" }));
}

#[test]
fn yaml_rule_switches_gate_both_public_lint_and_repair() {
    let config: Config = serde_norway::from_str(
        "rules:\n  comma_spacing: false\n  trailing_whitespace: false\n  identifier_casing: false\n",
    )
    .unwrap();
    let source = "Function run(Int left,Int right)  \nEndFunction\n";

    let diagnostics = lint(source, &config);

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.rule,
            "comma-spacing" | "trailing-whitespace" | "identifier-casing"
        )
    }));
    assert_eq!(repair(source, &config), source);
}

#[test]
fn opt_in_rule_is_disabled_by_default_through_the_public_api() {
    let source = "ScriptName Example\n\nGlobalVariable Property Toggle Auto\n\nFunction Run()\n    If Toggle.GetValue() == 1.0\n        Toggle.SetValue(1.0)\n    EndIf\nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "global-variable-setvalue"));
}

#[test]
fn yaml_can_enable_an_opt_in_rule_through_the_public_api() {
    let config: Config =
        serde_norway::from_str("rules:\n  global_variable_setvalue: true\n").unwrap();
    let source = "ScriptName Example\n\nGlobalVariable Property Toggle Auto\n\nFunction Run()\n    If Toggle.GetValue() == 1.0\n        Toggle.SetValue(1.0)\n    Else\n        Toggle.SetValue(0.0)\n    EndIf\nEndFunction\n";

    let diagnostics: Vec<_> = lint(source, &config)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "global-variable-setvalue")
        .collect();

    assert_eq!(diagnostics.len(), 2);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (7, 1));
    assert!(diagnostics[0].message.contains("does not change the value"));
    assert_eq!((diagnostics[1].line, diagnostics[1].column), (9, 1));
    assert!(diagnostics[1]
        .message
        .contains("may be an unnecessary write"));
}
