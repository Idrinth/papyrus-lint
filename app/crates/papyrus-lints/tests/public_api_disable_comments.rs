//! Black-box tests for `@disable` / unused-disable behavior through the
//! crate-level lint entry point.

use papyrus_lints::{lint, Config};

#[test]
fn rule_specific_disable_comment_only_suppresses_the_named_rule() {
    let source = "ScriptName Example\n\nFunction Run(Int left,Int right) ; @disable comma-spacing   \nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.line == 3 && diagnostic.rule == "comma-spacing"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.line == 3 && diagnostic.rule == "trailing-whitespace"));
}

#[test]
fn bare_disable_comment_suppresses_all_findings_on_its_line_only() {
    let source = "ScriptName Example\n\nFunction Run(Int left,Int right) ; @disable   \nFunction Other(Int left,Int right)\nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.line != 3));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.line == 4 && diagnostic.rule == "comma-spacing"));
}

#[test]
fn disable_rule_ids_are_case_insensitive_and_accept_a_list() {
    let source = "ScriptName Example\n\nFunction Run(Int left,Int right) ; @disable COMMA-SPACING, identifier-CASING\nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.line != 3 || !matches!(diagnostic.rule, "comma-spacing" | "identifier-casing")
    }));
}

#[test]
fn public_lint_reports_unknown_and_untriggered_disable_directives() {
    let source = "ScriptName Example\n\nCall() ; @disable mystery-rule, comma-spacing\n";
    let mut config = Config::default();
    config.rules.unused_disable = true;

    let diagnostics = lint(source, &config);
    let unused: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "unused-disable")
        .collect();

    assert_eq!(unused.len(), 2);
    assert_eq!((unused[0].line, unused[0].column), (3, 19));
    assert!(unused[0].message.contains("mystery-rule"));
    assert!(unused[0].message.contains("unknown"));
    assert_eq!((unused[1].line, unused[1].column), (3, 33));
    assert!(unused[1].message.contains("comma-spacing"));
    assert!(unused[1].message.contains("does not produce"));
}

#[test]
fn public_lint_does_not_report_a_disable_that_suppresses_a_finding() {
    let source = "ScriptName Example\n\nCall(1,2) ; @disable comma-spacing\n";
    let mut config = Config::default();
    config.rules.unused_disable = true;

    let diagnostics = lint(source, &config);

    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.rule != "comma-spacing" && diagnostic.rule != "unused-disable"
    }));
}

#[test]
fn bare_disable_is_unused_only_when_its_line_has_no_findings() {
    let source = "ScriptName Example\n\nCall() ; @disable\nCall(1,2) ; @disable\n";
    let mut config = Config::default();
    config.rules.unused_disable = true;

    let diagnostics = lint(source, &config);
    let unused: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "unused-disable")
        .collect();

    assert_eq!(unused.len(), 1);
    assert_eq!((unused[0].line, unused[0].column), (3, 10));
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.line != 4));
}

#[test]
fn unused_disable_rule_can_be_disabled_without_affecting_suppression() {
    let mut config = Config::default();
    config.rules.unused_disable = false;
    let source = "ScriptName Example\n\nCall() ; @disable mystery-rule\nCall(1,2) ; @disable comma-spacing\n";

    let diagnostics = lint(source, &config);

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-disable"));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| { diagnostic.line != 4 || diagnostic.rule != "comma-spacing" }));
}

#[test]
fn disable_comment_suppresses_an_opt_in_rule_on_only_its_own_line() {
    let mut config = Config::default();
    config.rules.global_variable_setvalue = true;
    let source = "ScriptName Example\n\nGlobalVariable Property Toggle Auto\n\nFunction Run()\n    If Toggle.GetValue() == 1.0\n        Toggle.SetValue(1.0) ; @disable global-variable-setvalue\n    Else\n        Toggle.SetValue(0.0)\n    EndIf\nEndFunction\n";

    let diagnostics: Vec<_> = lint(source, &config)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "global-variable-setvalue")
        .collect();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 9);
}
