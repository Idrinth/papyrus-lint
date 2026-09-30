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
fn checks_nested_conditions_inside_expression_shapes() {
    let diagnostics = check(
        "ScriptName Example\n\nBool Function Accept(Bool value)\n    Return value\nEndFunction\n\nFunction Test(Int x, Bool[] values)\n    If Accept(x > 10 && x < 5)\n    EndIf\n    If !(x == 1 && x == 2)\n    EndIf\n    If values[(x > 8 && x < 3) as Int]\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics.iter().map(|diagnostic| diagnostic.line).collect::<Vec<_>>(),
        vec![8, 10, 12]
    );
}

#[test]
fn handles_negative_float_and_reversed_bounds() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Float value)\n    If value >= -1.5 && -2.0 > value\n    EndIf\n    If -1.5 <= value && value <= -1.5\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn compares_parent_and_object_member_references_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Example other)\n    If Parent.Count > 3 && Parent.Count < 1\n    EndIf\n    If other.Count > 3 && OTHER.count < 1\n    EndIf\n    If Self.Count > 3 && Parent.Count < 1\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn caps_diagnostics_for_a_long_contradictory_chain() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x == 1 && x == 2 && x == 3 && x == 4\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 3);
}

#[test]
fn ignores_non_numeric_and_non_comparison_clauses() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x, String name, Bool ready)\n    If ready && ready\n    EndIf\n    If name == \"one\" && name == \"two\"\n    EndIf\n    If x + 1 && x + 2\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
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
