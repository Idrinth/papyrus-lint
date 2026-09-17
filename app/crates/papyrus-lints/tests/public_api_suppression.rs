//! Black-box coverage for the public diagnostic-suppression APIs.

use papyrus_lints::{
    is_disabled, lint_with_external_arguments_and_extra_diagnostics, Config, Diagnostic,
    NoExternalSignatures,
};

fn diagnostic(line: usize, rule: &'static str) -> Diagnostic {
    Diagnostic {
        line,
        column: 1,
        message: "[warning] externally produced diagnostic".into(),
        rule,
    }
}

#[test]
fn line_directives_are_line_scoped_and_case_insensitive() {
    let source = "Call(1,2) ; @disable COMMA-SPACING\nCall(3,4)\n";

    assert!(is_disabled(source, 1, "comma-spacing"));
    assert!(is_disabled(source, 1, "COMMA-SPACING"));
    assert!(!is_disabled(source, 1, "trailing-whitespace"));
    assert!(!is_disabled(source, 2, "comma-spacing"));
}

#[test]
fn file_directives_apply_to_every_line_but_only_to_named_rules() {
    let source = "; @disable-file stale-compiled-output\nCall(1,2)\n";

    assert!(is_disabled(source, 1, "stale-compiled-output"));
    assert!(is_disabled(source, 200, "STALE-COMPILED-OUTPUT"));
    assert!(!is_disabled(source, 2, "comma-spacing"));
}

#[test]
fn bare_directives_disable_every_rule_in_their_scope() {
    let source = "Call(1,2) ; @disable\n; @disable-file\n";

    assert!(is_disabled(source, 1, "any-rule-at-all"));
    assert!(is_disabled(source, 99, "another-rule"));
}

#[test]
fn directive_like_text_outside_line_comments_is_ignored() {
    let source = concat!(
        "String value = \"; @disable comma-spacing\"\n",
        ";/ @disable-file comma-spacing /;\n",
        "{ @disable comma-spacing }\n",
        "Call(1,2) ; @disabled comma-spacing\n",
    );

    for line in 1..=4 {
        assert!(!is_disabled(source, line, "comma-spacing"));
    }
}

#[test]
fn merged_external_diagnostics_honor_line_and_file_directives() {
    let source = concat!(
        "ScriptName Example ; @disable-file stale-compiled-output\n",
        "Call() ; @disable script-filename-mismatch\n",
        "Call()\n",
    );
    let extras = vec![
        diagnostic(3, "stale-compiled-output"),
        diagnostic(2, "script-filename-mismatch"),
        diagnostic(3, "conflicting-script-versions"),
    ];

    let diagnostics = lint_with_external_arguments_and_extra_diagnostics(
        source,
        &Config::default(),
        &mut NoExternalSignatures,
        extras,
    );

    assert!(diagnostics
        .iter()
        .any(|item| item.rule == "conflicting-script-versions" && item.line == 3));
    assert!(diagnostics
        .iter()
        .all(|item| item.rule != "stale-compiled-output"));
    assert!(diagnostics
        .iter()
        .all(|item| item.rule != "script-filename-mismatch"));
}

#[test]
fn used_external_diagnostic_directives_do_not_trigger_unused_disable() {
    let source = concat!(
        "ScriptName Example ; @disable-file stale-compiled-output\n",
        "Call() ; @disable script-filename-mismatch\n",
    );
    let extras = vec![
        diagnostic(2, "stale-compiled-output"),
        diagnostic(2, "script-filename-mismatch"),
    ];
    let mut config = Config::default();
    config.rules.unused_disable = true;

    let diagnostics = lint_with_external_arguments_and_extra_diagnostics(
        source,
        &config,
        &mut NoExternalSignatures,
        extras,
    );

    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn unused_external_diagnostic_directives_report_their_public_rule_ids() {
    let source = concat!(
        "ScriptName Example ; @disable-file stale-compiled-output\n",
        "Call() ; @disable script-filename-mismatch\n",
    );
    let mut config = Config::default();
    config.rules.unused_disable = true;

    let diagnostics = lint_with_external_arguments_and_extra_diagnostics(
        source,
        &config,
        &mut NoExternalSignatures,
        Vec::new(),
    );
    let unused: Vec<_> = diagnostics
        .iter()
        .filter(|item| item.rule == "unused-disable")
        .collect();

    assert_eq!(unused.len(), 2);
    assert!(unused[0].message.contains("stale-compiled-output"));
    assert!(unused[1].message.contains("script-filename-mismatch"));
}
