use super::*;

#[test]
fn simple_function_has_baseline_complexity_of_one() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n",
        0,
        20,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("complexity of 1"));
}

#[test]
fn does_not_flag_functions_at_or_below_warning_threshold() {
    let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    EndIf\nEndFunction\n";

    // Complexity is 2 (baseline 1 + one If branch); default warning is 10.
    assert!(check(source, 10, 20).is_empty());
}

#[test]
fn flags_warning_level_between_thresholds() {
    let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    EndIf\nEndFunction\n";

    let diagnostics = check(source, 1, 20);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("complexity of 2"));
    assert_eq!(diagnostics[0].line, 3);
}

#[test]
fn flags_error_level_above_error_threshold() {
    let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    EndIf\nEndFunction\n";

    let diagnostics = check(source, 0, 1);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[error]"));
}

#[test]
fn counts_elseif_and_else_if_branches_and_while_loops() {
    let source = "ScriptName Example\n\nFunction Test()\n    If a\n        Int i = 1\n    ElseIf b\n        Int i = 2\n    Else\n        Int i = 3\n    EndIf\n    While c\n        Int i = 4\n    EndWhile\nEndFunction\n";

    // Baseline 1 + If branch + ElseIf branch + While = 4.
    let diagnostics = check(source, 3, 20);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("complexity of 4"));
}

#[test]
fn counts_short_circuit_logical_operators_in_conditions() {
    let source =
            "ScriptName Example\n\nFunction Test()\n    If a && b || c\n        Int i = 1\n    EndIf\nEndFunction\n";

    // Baseline 1 + If branch + && + || = 4.
    let diagnostics = check(source, 3, 20);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("complexity of 4"));
}

#[test]
fn checks_functions_declared_in_states_too() {
    let source = "ScriptName Example\n\nState Active\n    Function Test()\n        If a\n            Int i = 1\n        EndIf\n    EndFunction\nEndState\n";

    let diagnostics = check(source, 1, 20);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Test"));
}

#[test]
fn treats_an_error_threshold_below_warning_as_equal_to_warning() {
    let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    EndIf\nEndFunction\n";

    // Complexity is 2, above the warning threshold (1). A misconfigured
    // error threshold (0) below it is normalized up to 1, so this reads
    // [error] rather than silently going unflagged or downgrading to
    // [warning] the way the raw, contradictory pair would suggest.
    let diagnostics = check(source, 1, 0);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("(warning: 1, error: 1)"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check(
        "ScriptName Example\n\nFunction Test(\nEndFunction\n",
        10,
        20
    )
    .is_empty());
}

#[test]
fn counts_logical_operators_in_each_statement_expression() {
    let source = "ScriptName Example\n\nBool Function Test(Bool a, Bool b)\n    Bool value = a && b\n    value = a || b\n    Consume(a && b)\n    Return a || b\nEndFunction\n";

    // Baseline 1 plus one short-circuit operator in each of the four
    // statement expression forms above.
    let diagnostics = check(source, 4, 20);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("complexity of 5"));
}

#[test]
fn counts_nested_control_flow_inside_else_bodies() {
    let source = "ScriptName Example\n\nFunction Test()\n    If ready\n        Return\n    Else\n        While waiting\n            If failed\n                Return\n            EndIf\n        EndWhile\n    EndIf\nEndFunction\n";

    // Else itself adds no path, but its nested While and If do.
    let diagnostics = check(source, 3, 20);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("complexity of 4"));
}

#[test]
fn reports_each_over_threshold_function_independently() {
    let source = "ScriptName Example\n\nFunction First()\n    If ready\n    EndIf\nEndFunction\n\nEvent OnInit()\n    While waiting\n    EndWhile\nEndEvent\n";

    let diagnostics = check(source, 1, 20);

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.line, diagnostic.column))
            .collect::<Vec<_>>(),
        vec![(3, 1), (8, 1)]
    );
    assert!(diagnostics[0].message.contains("'First'"));
    assert!(diagnostics[1].message.contains("'OnInit'"));
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule == RULE));
}
