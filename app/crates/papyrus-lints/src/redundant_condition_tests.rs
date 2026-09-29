use super::*;

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
fn flags_dominated_and_clause() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x > 10 && x > 5\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("redundant"));
}

#[test]
fn flags_duplicate_or_equality() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x == 5 || x == 5\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn flags_dominated_or_clause() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x > 10 || x > 5\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_duplicate_identifier_clause() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag || flag\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_three_clause_and_chain() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x > 20 && x > 10 && x > 5\n    EndIf\nEndFunction\n",
    );

    // x>10 and x>5 are both redundant given x>20 (and x>5 also given x>10).
    assert!(diagnostics.len() >= 2);
    assert!(diagnostics.iter().all(|d| d.rule == RULE));
}

#[test]
fn flags_while_and_elseif_conditions() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    While x >= 10 && x > 5\n        Return\n    EndWhile\n    If x < 0\n    ElseIf x <= 3 && x < 10\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn normalizes_literal_on_the_left() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If 10 < x && 5 < x\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn matches_member_property_access() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    If Self.Health > 10 && Self.Health > 5\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_independent_thresholds() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x > 5 && x < 10\n    EndIf\n    If x > 5 || x < 2\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_different_identifiers() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x, Int y)\n    If x > 10 && y > 5\n    EndIf\n    If x > 10 || y > 5\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_calls_or_aliases() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If GetValue() > 10 && GetValue() > 5\n    EndIf\n    If x > 10 && GetValue() > 5\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_single_comparisons() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x, Bool flag)\n    If x > 10\n    EndIf\n    If flag\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_nested_logic_separately() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x, Bool flag)\n    If (x > 10 && x > 5) || flag\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn respects_config_off_switch() {
    let source =
        "ScriptName Example\n\nFunction Test(Int x)\n    If x > 10 && x > 5\n    EndIf\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.redundant_condition = false;
    let diagnostics = crate::lint(source, &config);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}
