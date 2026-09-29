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
fn flags_impossible_range_and() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x > 10 && x < 5\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("contradict"));
}

#[test]
fn flags_conflicting_equalities() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x == 1 && x == 2\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_exclusive_bounds_at_same_value() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x > 5 && x < 5\n    EndIf\n    If x > 5 && x <= 5\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn flags_while_and_elseif_conditions() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    While x >= 10 && x < 3\n        Return\n    EndWhile\n    If x == 0\n    ElseIf x == 1 && x == 2\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn normalizes_literal_on_the_left() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If 10 < x && x < 5\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn matches_member_property_access() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    If Self.Health > 10 && Self.Health < 5\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_compatible_and_ranges() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x > 5 && x < 10\n    EndIf\n    If x >= 5 && x <= 5\n    EndIf\n    If x > 10 && x > 5\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_or_chains() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x > 10 || x < 5\n    EndIf\n    If x == 1 || x == 2\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_different_identifiers() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x, Int y)\n    If x > 10 && y < 5\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_calls() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If GetValue() > 10 && GetValue() < 5\n    EndIf\n    If x > 10 && GetValue() < 5\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_nested_and_inside_or() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x, Bool flag)\n    If (x > 10 && x < 5) || flag\n    EndIf\nEndFunction\n",
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
        "ScriptName Example\n\nFunction Test(Int x)\n    If x > 10 && x < 5\n    EndIf\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.contradictory_condition = false;
    let diagnostics = crate::lint(source, &config);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}
