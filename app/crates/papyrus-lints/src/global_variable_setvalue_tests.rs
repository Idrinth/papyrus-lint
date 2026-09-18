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
fn flags_unguarded_else_write_on_a_receiver_the_chain_reads() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n        gv.SetValue(2.0)\n    Else\n        gv.SetValue(0.0)\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("gv.SetValue(0)"));
    assert!(diagnostics[0].message.contains("ElseIf gv.GetValue() != 0"));
}

#[test]
fn flags_a_branch_that_writes_back_the_value_its_own_condition_confirmed() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 2.0\n        gv.SetValue(2.0)\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert!(diagnostics[0].message.contains("does not change the value"));
}

#[test]
fn does_not_flag_a_branch_that_writes_a_different_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n        gv.SetValue(2.0)\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_the_corrected_pattern_using_a_saved_local_and_explicit_elseif() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    Float current = gv.GetValue()\n    If current == 1.0\n        gv.SetValue(2.0)\n    ElseIf current != 0.0\n        gv.SetValue(0.0)\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_else_write_on_an_unrelated_receiver() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, GlobalVariable other)\n    If gv.GetValue() == 1.0\n        gv.SetValue(2.0)\n    Else\n        other.SetValue(0.0)\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_setvalue_call_guarded_by_its_own_nested_if() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n        gv.SetValue(2.0)\n    Else\n        If gv.GetValue() != 0.0\n            gv.SetValue(0.0)\n        EndIf\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_mixed_int_and_float_forms_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.getvalueint() == 1\n        gv.setvalue(1.0)\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_condition_depending_on_more_than_a_bare_equality() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Bool flag)\n    If gv.GetValue() == 1.0 && flag\n        gv.SetValue(1.0)\n    Else\n        gv.SetValue(0.0)\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(GlobalVariable gv)\n        If gv.GetValue() == 2.0\n            gv.SetValue(2.0)\n        EndIf\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn checks_an_if_nested_inside_a_while_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    While gv.GetValue() == 1.0\n        If gv.GetValue() == 1.0\n            gv.SetValue(1.0)\n        EndIf\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_conditions_that_are_not_a_bare_receiver_getter_equality() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, GlobalVariable other)\n    If gv.GetValue(1) == 1.0\n        gv.SetValue(1.0)\n    ElseIf GetValue() == 1.0\n        gv.SetValue(1.0)\n    ElseIf gv.GetSomethingElse() == 1.0\n        gv.SetValue(1.0)\n    ElseIf gv.GetValue() == other\n        gv.SetValue(1.0)\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_condition_comparing_against_a_non_numeric_literal() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == \"not-a-number\"\n        gv.SetValue(1.0)\n    ElseIf gv.GetValue() == true\n        gv.SetValue(1.0)\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_statements_in_the_branch_body_that_are_not_a_matching_setvalue_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n        gv.OtherProperty\n        gv.GetValue()\n        gv.SetValue(1.0, 2.0)\n        SetValue(1.0)\n        gv.SetSomethingElse(1.0)\n        gv.SetValue(notALiteral)\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn resolves_self_and_chained_member_receivers() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(SomeQuest quest)\n    If Self.GetValue() == 1.0\n        Self.SetValue(1.0)\n    EndIf\n    If quest.MyGlobal.GetValue() == 2.0\n        quest.MyGlobal.SetValue(2.0)\n    EndIf\nEndFunction\n",
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
fn does_not_flag_a_receiver_that_is_neither_an_identifier_self_nor_member_chain() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable[] gvs)\n    If gvs[0].GetValue() == 1.0\n        gvs[0].SetValue(1.0)\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn literal_to_f64_rejects_non_numeric_literals() {
    assert_eq!(literal_to_f64(&Literal::String("x".to_string())), None);
    assert_eq!(literal_to_f64(&Literal::Bool(true)), None);
    assert_eq!(literal_to_f64(&Literal::None), None);
}

#[test]
fn literal_display_is_empty_for_non_numeric_literals() {
    assert_eq!(literal_display(&Literal::String("x".to_string())), "");
    assert_eq!(literal_display(&Literal::Bool(true)), "");
    assert_eq!(literal_display(&Literal::None), "");
}

#[test]
fn receiver_key_and_display_reject_expressions_that_are_not_a_simple_chain() {
    let not_a_receiver = Expr::Literal(Literal::int(1));
    assert_eq!(receiver_key(&not_a_receiver), None);
    assert_eq!(receiver_display(&not_a_receiver), None);
}
