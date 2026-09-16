use super::*;

#[test]
fn flags_elseif_already_covered_by_a_stricter_earlier_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x)\n    If x > 9\n    ElseIf x > 10\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("line 4"));
}

#[test]
fn flags_elseif_covered_by_a_geq_earlier_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x)\n    If x >= 9\n    ElseIf x > 9\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_duplicate_equality_condition() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x)\n    If x == 5\n    ElseIf x == 5\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_genuinely_reachable_elseif() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x)\n    If x > 9\n    ElseIf x < 3\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_elseif_a_geq_earlier_branch_does_not_cover() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x)\n    If x > 9\n    ElseIf x >= 9\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_reachable_elseif_between_two_other_branches() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x)\n    If x > 20\n    ElseIf x > 15\n    ElseIf x > 10\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_conditions_on_different_expressions() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x, Int y)\n    If x > 9\n    ElseIf y > 10\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_compound_or_non_numeric_conditions() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x, Bool flag)\n    If x > 9 && flag\n    ElseIf x > 10\n    EndIf\n    If flag\n    ElseIf x > 1\n    EndIf\n    If x != 9\n    ElseIf x > 10\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn normalizes_a_literal_written_on_the_left() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x)\n    If 9 < x\n    ElseIf x > 10\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn normalizes_negated_literals_and_inclusive_upper_bounds() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x)\n    If -5 >= x\n    ElseIf x < -6\n    EndIf\n    If -5 > x\n    ElseIf x <= -6\n    EndIf\nEndFunction\n",
        );

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        [5, 8]
    );
}

#[test]
fn skips_non_interval_current_and_earlier_branches() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x)\n    If x != 5\n    ElseIf x > 10\n    EndIf\n    If x > 5\n    ElseIf x != 10\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_the_same_member_access_expression() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If Self.Health > 9\n    ElseIf Self.Health > 10\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn checks_nested_and_state_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Int x)\n        While true\n            If x > 9\n            ElseIf x > 10\n            EndIf\n        EndWhile\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}
