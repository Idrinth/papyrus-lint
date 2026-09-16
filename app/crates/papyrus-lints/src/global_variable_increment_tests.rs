use super::*;

#[test]
fn flags_getvalue_plus_other_operand() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].column, 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[info]"));
    assert!(diagnostics[0].message.contains("gv.Mod(x)"));
}

#[test]
fn flags_other_operand_plus_getvalue() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(x + gv.GetValue())\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.setvalue(gv.getvalue() + x)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_setvalueint() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int x)\n    gv.SetValueInt(gv.GetValueInt() + x)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_different_operator() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() - x)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_getvalue_call_on_a_different_receiver() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, GlobalVariable other)\n    gv.SetValue(other.GetValue() + 1.0)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_plain_setvalue_with_no_addition() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    gv.SetValue(1.0)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_addition_of_two_unrelated_values() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float a, Float b)\n    gv.SetValue(a + b)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn resolves_self_and_chained_member_receivers() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(SomeQuest quest, Float x)\n    Self.SetValue(Self.GetValue() + x)\n    quest.MyGlobal.SetValue(quest.MyGlobal.GetValue() + x)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("Self.Mod")));
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("quest.MyGlobal.Mod")));
}

#[test]
fn does_not_flag_a_receiver_that_is_neither_an_identifier_self_nor_member_chain() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable[] gvs, Int i, Float x)\n    gvs[i].SetValue(gvs[i].GetValue() + x)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(GlobalVariable gv, Float x)\n        gv.SetValue(gv.GetValue() + x)\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn checks_nested_if_while_and_else_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x, Bool cond)\n    While cond\n        If cond\n            gv.SetValue(gv.GetValue() + x)\n        Else\n            gv.SetValue(x + gv.GetValue())\n        EndIf\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn does_not_flag_a_call_with_more_than_one_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x, x)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_unqualified_setvalue_call() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Float x)\n    SetValue(GetValue() + x)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn receiver_key_and_display_reject_expressions_that_are_not_a_simple_chain() {
    let not_a_receiver = Expr::Literal(papyrus_parser::ast::Literal::int(1));
    assert_eq!(receiver_key(&not_a_receiver), None);
    assert_eq!(receiver_display(&not_a_receiver), None);
}

#[test]
fn repairs_getvalue_plus_other_operand() {
    let repaired = repair(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x)\nEndFunction\n",
        );

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.Mod(x)\nEndFunction\n"
        );
    assert!(check(&repaired).is_empty());
}

#[test]
fn repairs_other_operand_plus_getvalue() {
    let repaired = repair(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(x + gv.GetValue())\nEndFunction\n",
        );

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.Mod(x)\nEndFunction\n"
        );
}

#[test]
fn repairs_case_insensitively_and_preserves_the_receivers_own_casing() {
    let repaired = repair(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.setvalue(gv.getvalue() + x)\nEndFunction\n",
        );

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.Mod(x)\nEndFunction\n"
        );
}

#[test]
fn repairs_self_and_chained_member_receivers() {
    let repaired = repair(
            "ScriptName Example\n\nFunction Test(SomeQuest quest, Float x)\n    Self.SetValue(Self.GetValue() + x)\n    quest.MyGlobal.SetValue(quest.MyGlobal.GetValue() + x)\nEndFunction\n",
        );

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(SomeQuest quest, Float x)\n    Self.Mod(x)\n    quest.MyGlobal.Mod(x)\nEndFunction\n"
        );
}

#[test]
fn repairs_a_complex_addend_and_keeps_its_own_spacing() {
    let repaired = repair(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    gv.SetValue(gv.GetValue() + GetAmount(1, 2))\nEndFunction\n",
        );

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    gv.Mod(GetAmount(1, 2))\nEndFunction\n"
        );
}

#[test]
fn repairs_multiple_calls_and_nested_bodies() {
    let repaired = repair(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x, Bool cond)\n    While cond\n        If cond\n            gv.SetValue(gv.GetValue() + x)\n        Else\n            gv.SetValue(x + gv.GetValue())\n        EndIf\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x, Bool cond)\n    While cond\n        If cond\n            gv.Mod(x)\n        Else\n            gv.Mod(x)\n        EndIf\n    EndWhile\nEndFunction\n"
        );
}

#[test]
fn repair_leaves_setvalueint_untouched() {
    let source =
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int x)\n    gv.SetValueInt(gv.GetValueInt() + x)\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_a_different_operator_untouched() {
    let source =
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() - x)\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_a_call_with_more_than_one_argument_untouched() {
    let source =
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x, x)\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_unrelated_lines_and_other_statements_untouched() {
    let source = "ScriptName Example\n\nFunction Test(GlobalVariable gv, GlobalVariable other, Float x)\n    gv.SetValue(1.0)\n    other.SetValue(gv.GetValue() + x)\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn does_not_crash_repairing_unparseable_source() {
    let source = "ScriptName Example\n\nFunction Test(\nEndFunction\n";
    assert_eq!(repair(source), source);
}
