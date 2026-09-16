use super::*;

#[test]
fn flags_the_same_global_read_across_an_elseif_chain() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n    ElseIf gv.GetValue() == 2.0\n    ElseIf gv.GetValue() == 3.0\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[1].line, 6);
    assert!(diagnostics.iter().all(|d| d.rule == RULE));
    assert!(diagnostics
        .iter()
        .all(|d| d.message.contains("read it into a local variable")));
}

#[test]
fn does_not_flag_different_receivers() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gvA, GlobalVariable gvB)\n    If gvA.GetValue() == 1.0\n    ElseIf gvB.GetValue() == 2.0\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_single_read() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_different_method_name() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValueInt() == 1\n    ElseIf gv.GetValueInt() == 2\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_call_taking_arguments() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetValue(\"Health\") == 1.0\n    ElseIf akActor.GetValue(\"Health\") == 2.0\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_repeated_read_combined_with_logical_operators_in_one_condition() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0 || gv.GetValue() == 2.0\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn checks_nested_and_state_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(GlobalVariable gv)\n        If gv.GetValue() == 1.0\n            If gv.GetValue() == 2.0\n            ElseIf gv.GetValue() == 3.0\n            EndIf\n        EndIf\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn checks_an_if_chain_nested_inside_a_while_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    While true\n        If gv.GetValue() == 1.0\n        ElseIf gv.GetValue() == 2.0\n        EndIf\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn ignores_non_if_statements_around_the_chain() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    Float x = 0.0\n    x = 1.0\n    Debug.Trace(\"hi\")\n    If gv.GetValue() == 1.0\n    ElseIf gv.GetValue() == 2.0\n    EndIf\n    Return\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_an_unqualified_get_value_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If GetValue() == 1.0\n    ElseIf GetValue() == 2.0\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_repeated_read_wrapped_in_a_negation() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If !(gv.GetValue() == 1.0)\n    ElseIf !(gv.GetValue() == 2.0)\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_repeated_read_passed_as_a_named_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If SomeCheck(threshold = gv.GetValue()) == 1.0\n    ElseIf SomeCheck(threshold = gv.GetValue()) == 2.0\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_repeated_read_on_an_indexed_receiver() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable[] gvs, Int i)\n    If gvs[i].GetValue() == 1.0\n    ElseIf gvs[i].GetValue() == 2.0\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_repeated_read_wrapped_in_a_cast() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If (gv.GetValue() as Float) > 1.0\n    ElseIf (gv.GetValue() as Float) > 2.0\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn walks_a_new_array_size_expression_without_crashing() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If (new Int[gv.GetValue() as Int]) == None\n    ElseIf (new Int[gv.GetValue() as Int]) == None\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}
