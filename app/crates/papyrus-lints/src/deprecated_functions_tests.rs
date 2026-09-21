use super::*;

#[test]
fn recognizes_deprecated_with_replacement_before_other_annotations() {
    assert!(line_has_deprecated(
        "Function Old() ; @deprecated Use New() @nodiscard @public"
    ));
}

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

#[test]
fn ignores_comments_strings_and_identifiers_that_are_not_calls() {
    let diagnostics = check("String value = \"Spell.Preload()\" ; Spell.Preload()\nInt Preload = 1\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_calls_to_a_locally_deprecated_function() {
    let diagnostics = check(
        "ScriptName Example\n\n; @deprecated\nFunction OldWay()\nEndFunction\n\nFunction Test()\n    OldWay()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (8, 5));
    assert_eq!(
        diagnostics[0].message,
        "[warning] Function 'OldWay' is marked deprecated"
    );
}

#[test]
fn supports_a_trailing_deprecated_directive_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\nFunction OldWay() ; @DePrEcAtEd\nEndFunction\nFunction Test()\n    Self.OldWay()\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn deprecated_word_must_be_bounded_and_inside_a_comment() {
    let diagnostics = check(
        "ScriptName Example\nFunction Similar(String value = \"; @deprecated\") ; @deprecatedSoon\nEndFunction\nFunction Test()\n    Similar()\nEndFunction\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn disable_directives_suppress_findings() {
    let source = "; @disable-file deprecated-functions\nScriptName Example\nFunction Test(Spell akSpell)\n  akSpell.Preload()\nEndFunction\n";
    let diagnostics = crate::lint(source, &crate::config::Config::default());
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_switch_disables_rule_through_dispatch() {
    let source = "ScriptName Example\nFunction Test(Spell akSpell)\n  akSpell.Preload()\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.deprecated_functions = false;

    let diagnostics = crate::lint(source, &config);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}
