use super::*;

#[test]
fn flags_variable_read_before_any_assignment() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'i'"));
}

#[test]
fn does_not_flag_variable_declared_with_an_initial_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 0\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_variable_assigned_before_use() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    i = 1\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_compound_assignment_reading_an_unassigned_variable() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int i\n    i += 1\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_after_compound_assignment_establishes_a_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    i += 1\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn flags_read_inside_the_initializer_of_another_declaration() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int i\n    Int j = i\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert!(diagnostics[0].message.contains("'i'"));
}

#[test]
fn flags_read_through_a_member_access() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Armor a\n    a.GetName()\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_after_both_if_branches_assign() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    If flag\n        i = 1\n    Else\n        i = 2\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_use_after_one_sided_if_assigns_it() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    If flag\n        i = 1\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_use_when_only_one_branch_of_several_assigns() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag, Bool other)\n    Int i\n    If flag\n        Debug.Trace(\"a\")\n    ElseIf other\n        i = 1\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn still_flags_use_after_if_else_when_neither_branch_assigns() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    If flag\n        Debug.Trace(\"a\")\n    Else\n        Debug.Trace(\"b\")\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 10);
}

#[test]
fn does_not_flag_the_conditional_assign_then_default_check_idiom() {
    // The reported false positive (papyrus-lint#363): a variable is
    // assigned in only one branch, then a *later*, separate `If`
    // compares it against its default to find out whether that branch
    // ran, deliberately relying on the language's implicit default.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag, Bool sigilStoneInstalled)\n    Bool daedricItemCrafted\n    If flag\n        daedricItemCrafted = True\n    EndIf\n    If sigilStoneInstalled == False || daedricItemCrafted == False\n        Debug.Trace(\"no recipes\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_every_branch_returns_or_assigns() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    If flag\n        Return\n    Else\n        i = 2\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_a_guard_clause_assigns_before_falling_through() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    If flag\n        i = 1\n    Else\n        Return\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_use_after_while_loop_since_it_may_run_zero_times() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    While flag\n        i = 1\n        flag = false\n    EndWhile\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 9);
}

#[test]
fn does_not_flag_function_parameters() {
    let diagnostics =
            check("ScriptName Example\n\nFunction Test(Int count)\n    Debug.Trace(count as String)\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_script_properties() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction Test()\n    Debug.Trace(MyValue as String)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_variable_usage_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int total\n    Debug.Trace(TOTAL as String)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Int i\n        Debug.Trace(i as String)\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'i'"));
}

#[test]
fn each_function_starts_fresh() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction First()\n    Int i\n    i = 1\nEndFunction\n\nFunction Second()\n    Int i\n    Debug.Trace(i as String)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 10);
}

#[test]
fn flags_passing_an_unassigned_variable_as_a_call_argument() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int i\n    Debug.Trace(i)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn does_not_flag_a_gate_comparison_against_the_int_default() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If i == 0\n        i = 5\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_gate_comparison_with_the_default_literal_on_the_left() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If 0 == i\n        i = 5\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_not_equal_gate_comparison_against_the_default() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If i != 0\n        Debug.Trace(\"set\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_gate_comparison_against_none() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a\n    If a == None\n        Return\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_gate_comparison_against_the_float_default() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f\n    If f == 0.0\n        f = 1.0\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_gate_comparison_against_the_bool_default() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Bool b\n    If b == False\n        b = True\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_gate_comparison_against_the_string_default() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    String s\n    If s == \"\"\n        s = \"set\"\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn still_flags_a_comparison_against_a_non_default_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If i == 5\n        i = 5\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn still_flags_a_genuine_read_alongside_an_unrelated_gate_comparison() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If i == 0\n        Debug.Trace(i as String)\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn still_flags_an_int_compared_against_none_since_that_is_not_its_default() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If i == None\n        i = 5\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn still_flags_a_bool_compared_against_zero_since_that_is_not_its_default() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Bool b\n    If b == 0\n        b = True\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_an_or_joined_default_gate_guarding_a_while_condition() {
    // papyrus-lint#457: `a == None || a.IsDead()` only evaluates
    // `a.IsDead()` once the gate has established `a` is no longer at
    // its default, so it shouldn't be flagged as a read of `a` before
    // assignment.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    While a == None || a.IsDead()\n        a = Game.GetPlayer()\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_or_joined_default_gate_guarding_an_if_condition() {
    // Same shape as above, but in a plain `If` rather than a `While`
    // condition, confirming the fix isn't loop-specific.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    If a == None || a.IsDead()\n        a = Game.GetPlayer()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_and_joined_default_gate_the_other_polarity() {
    // The `&&` polarity: `a != None && a.IsDead()` only evaluates
    // `a.IsDead()` once the gate has established `a` is no longer at
    // its default.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    If a != None && a.IsDead()\n        a = Game.GetPlayer()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn still_flags_an_or_joined_operand_when_the_gate_does_not_rule_out_the_default() {
    // `a != None || a.IsDead()`: the gate being false (needed to reach
    // `a.IsDead()`) means `a == None`, so the default is *not* ruled
    // out and the read is still a genuine bug.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    If a != None || a.IsDead()\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn still_flags_an_and_joined_operand_when_the_gate_does_not_rule_out_the_default() {
    // `a == None && a.IsDead()`: the gate being true (needed to reach
    // `a.IsDead()`) means `a == None`, so the default is *not* ruled
    // out and the read is still a genuine bug.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    If a == None && a.IsDead()\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_a_read_inside_an_and_joined_default_gate_branch_body() {
    // papyrus-lint: `found != None && !found.IsDead()` rules out
    // `found`'s default before the branch body runs, so reading
    // `found` inside that body (not just within the condition itself)
    // shouldn't be flagged either.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor found\n    If found != None && !found.IsDead()\n        Float new_x = found.GetPositionX()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_read_in_the_else_body_of_a_single_default_gate_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor found\n    If found != None\n        Debug.Trace(\"found\")\n    Else\n        found.GetPositionX()\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 8);
}

#[test]
fn does_not_flag_a_chain_of_or_joined_default_gates_before_a_final_read() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    Actor b\n    If a == None || b == None || a.IsDead()\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}
