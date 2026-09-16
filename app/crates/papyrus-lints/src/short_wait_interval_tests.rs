use super::*;

#[test]
fn flags_wait_below_the_default_minimum() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.05)\nEndFunction\n",
        0.1,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Wait(0.05)"));
}

#[test]
fn does_not_flag_wait_at_or_above_the_minimum() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.1)\n    Utility.Wait(1.0)\nEndFunction\n",
            0.1,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_unqualified_wait() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Wait(0.01)\nEndFunction\n",
        0.1,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_wait_on_an_unrelated_script() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(MyScript akOther)\n    akOther.Wait(0.01)\nEndFunction\n",
            0.1,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_unqualified_register_for_update() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    RegisterForSingleUpdate(0.02)\nEndFunction\n",
        0.1,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("RegisterForSingleUpdate(0.02)"));
}

#[test]
fn flags_register_for_update_on_an_arbitrary_receiver() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    akRef.RegisterForUpdate(0.05)\nEndFunction\n",
            0.1,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("RegisterForUpdate"));
}

#[test]
fn flags_register_for_update_game_time_variants() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    RegisterForUpdateGameTime(0.05)\n    RegisterForSingleUpdateGameTime(0.05)\nEndFunction\n",
            0.1,
        );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn does_not_flag_register_for_update_at_or_above_the_minimum() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    RegisterForSingleUpdate(0.1)\nEndFunction\n",
        0.1,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn honors_a_configured_minimum() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.4)\nEndFunction\n",
        0.5,
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_constant_expression_that_folds_below_the_minimum() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.2 - 0.15)\nEndFunction\n",
        0.1,
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_named_argument() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Utility.Wait(afSeconds = 0.01)\nEndFunction\n",
        0.1,
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_runtime_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Float afSeconds)\n    Utility.Wait(afSeconds)\n    Utility.Wait(GetInterval())\nEndFunction\n",
            0.1,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_conditions_return_values_and_nested_state_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        If true\n            Utility.Wait(0.01)\n        EndIf\n    EndFunction\nEndState\n",
            0.1,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n", 0.1).is_empty());
}

#[test]
fn does_not_crash_on_a_wait_call_with_no_arguments() {
    assert!(check(
        "ScriptName Example\n\nFunction Test()\n    Utility.Wait()\nEndFunction\n",
        0.1
    )
    .is_empty());
}

#[test]
fn walks_extra_arguments_beyond_the_interval_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    RegisterForUpdate(0.05, GetValue())\nEndFunction\n",
            0.1,
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_wait_call_qualified_by_a_non_identifier_expression() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    GetUtility().Wait(0.01)\nEndFunction\n",
        0.1,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_call_whose_callee_is_neither_an_identifier_nor_a_member_access() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Self(0.01)\nEndFunction\n",
        0.1,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_constant_addition_that_folds_below_the_minimum() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Utility.Wait(1 + 2)\nEndFunction\n",
        10.0,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Wait(3)"));
}

#[test]
fn flags_a_constant_multiplication_that_folds_below_the_minimum() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.01 * 2)\nEndFunction\n",
        0.1,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Wait(0.02)"));
}

#[test]
fn walks_array_index_cast_and_new_array_expressions_without_crashing() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int[] values)\n    Int x = values[0]\n    Int y = 5 as Int\n    Int[] arr = new Int[3]\nEndFunction\n",
            0.1,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn as_number_rejects_a_non_numeric_literal() {
    assert_eq!(as_number(&Literal::Bool(true)), None);
}
