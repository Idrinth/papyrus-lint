use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check(ast.as_ref())
}

#[test]
fn flags_a_setvalue_call_directly_in_a_while_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        gv.SetValue(a * 33)\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("gv.SetValue(...)"));
}

#[test]
fn does_not_flag_a_setvalue_call_outside_any_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    gv.SetValue(1.0)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_when_the_loop_also_calls_utility_wait() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        gv.SetValue(a * 33)\n        Utility.Wait(1.0)\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_when_the_loop_also_calls_register_for_update() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        gv.SetValue(a * 33)\n        RegisterForSingleUpdate(1.0)\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_setvalue_call_nested_inside_an_if_within_the_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        If a > 5\n            gv.SetValue(a * 33)\n        EndIf\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn flags_a_setvalue_call_in_an_else_branch_within_the_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        If a > 5\n        Else\n            gv.SetValue(a * 33)\n        EndIf\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_calls_in_every_branch_in_source_order() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        If a > 10\n            gv.SetValue(10.0)\n        ElseIf a > 5\n            gv.SetValueInt(5)\n        Else\n            gv.SetValue(0.0)\n        EndIf\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        vec![6, 8, 10]
    );
    assert!(diagnostics[1].message.contains("gv.SetValueInt(...)"));
}

#[test]
fn a_wait_in_an_if_condition_paces_the_containing_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        gv.SetValue(a)\n        If Utility.Wait(1.0)\n            a -= 1\n        EndIf\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn a_wait_in_a_nested_loop_condition_paces_the_outer_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        gv.SetValue(a)\n        While Utility.Wait(1.0)\n            a -= 1\n        EndWhile\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn recognizes_wait_calls_nested_in_other_expressions() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        gv.SetValue(a)\n        Bool paced = !Utility.Wait(1.0)\n        a = values[Utility.Wait(1.0) as Int]\n        Return Wrapper(Utility.Wait(1.0))\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn only_flags_setvalue_calls_that_are_standalone_statements() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        Float result = gv.SetValue(a)\n        result = gv.SetValueInt(a)\n        Consume(gv.SetValue(a))\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_setvalueint_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        gv.setvalueint(a)\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("setvalueint"));
}

#[test]
fn does_not_flag_a_different_method_name() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        gv.SetSomethingElse(a)\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_getvalue_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        gv.GetValue()\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_nested_and_state_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(GlobalVariable gv, Int a)\n        While a > 0\n            gv.SetValue(a)\n            a -= 1\n        EndWhile\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn checks_each_nested_loop_independently() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gvA, GlobalVariable gvB, Int a, Int b)\n    While a > 0\n        gvA.SetValue(a)\n        While b > 0\n            gvB.SetValue(b)\n            b -= 1\n        EndWhile\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn a_wait_call_only_paces_the_loop_it_is_directly_in() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gvA, GlobalVariable gvB, Int a, Int b)\n    While a > 0\n        gvA.SetValue(a)\n        While b > 0\n            gvB.SetValue(b)\n            Utility.Wait(1.0)\n            b -= 1\n        EndWhile\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("gvA.SetValue"));
}

#[test]
fn does_not_flag_an_unqualified_setvalue_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        SetValue(a)\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn resolves_self_and_chained_member_receivers() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(SomeQuest quest, Int a)\n    While a > 0\n        Self.SetValue(a)\n        quest.MyGlobal.SetValue(a)\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Self.SetValue")));
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("quest.MyGlobal.SetValue")));
}

#[test]
fn flags_an_indexed_receiver_with_a_generic_display() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable[] gvs, Int a)\n    While a > 0\n        gvs[0].SetValue(a)\n        a -= 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("This receiver.SetValue"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}
