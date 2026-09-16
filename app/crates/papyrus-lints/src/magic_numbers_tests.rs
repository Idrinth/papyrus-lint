use super::*;

#[test]
fn flags_a_literal_used_directly_in_a_call_argument() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    DoThing(42)\nEndFunction\n",
        MagicNumbers::Loose,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("42"));
}

#[test]
fn does_not_flag_ignored_values() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    DoThing(0)\n    DoThing(1)\n    DoThing(-1)\nEndFunction\n",
            MagicNumbers::Loose,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_ignored_float_values() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    DoThing(0.0)\n    DoThing(1.0)\n    DoThing(-1.0)\nEndFunction\n",
            MagicNumbers::Loose,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_non_ignored_negative_literal() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    DoThing(-5)\nEndFunction\n",
        MagicNumbers::Loose,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("-5"));
}

#[test]
fn does_not_flag_a_bare_literal_local_variable_declaration() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int kMaxTargets = 5\nEndFunction\n",
        MagicNumbers::Loose,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_bare_literal_reassignment() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int kMaxTargets = 5\n    kMaxTargets = 6\nEndFunction\n",
            MagicNumbers::Loose,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_literal_nested_in_a_declaration_initializer() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int kMaxTargets = 5 + 1\nEndFunction\n",
        MagicNumbers::Loose,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains('5'));
}

#[test]
fn does_not_flag_a_bare_literal_script_variable_declaration() {
    let diagnostics = check(
        "ScriptName Example\n\nInt kMaxTargets = 5\n\nFunction Test()\nEndFunction\n",
        MagicNumbers::Loose,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_property_defaults() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property MaxTargets = 5 Auto\n",
        MagicNumbers::Loose,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn loose_mode_does_not_flag_wait_or_register_for_update_intervals() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(5)\n    RegisterForSingleUpdate(5)\nEndFunction\n",
            MagicNumbers::Loose,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn loose_mode_exemption_does_not_reach_through_a_nested_call() {
    // The exemption covers Wait's own interval argument, not a call
    // nested inside it.
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Utility.Wait(SomeCall(5))\nEndFunction\n",
        MagicNumbers::Loose,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains('5'));
}

#[test]
fn loose_mode_exemption_covers_an_arithmetic_interval_expression() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Utility.Wait(5 + 2)\nEndFunction\n",
        MagicNumbers::Loose,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn strict_mode_flags_wait_and_register_for_update_intervals() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(5)\n    RegisterForSingleUpdate(5)\nEndFunction\n",
            MagicNumbers::Strict,
        );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn checks_conditions_and_nested_state_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        If GetValue() > 42\n            DoThing()\n        EndIf\n    EndFunction\nEndState\n",
            MagicNumbers::Loose,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check(
        "ScriptName Example\n\nFunction Test(\nEndFunction\n",
        MagicNumbers::Loose
    )
    .is_empty());
}

#[test]
fn flags_a_magic_number_directly_returned() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Function Test()\n    Return 42\nEndFunction\n",
        MagicNumbers::Loose,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("42"));
}

#[test]
fn does_not_flag_a_bare_return_with_no_value() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Return\nEndFunction\n",
        MagicNumbers::Loose,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_magic_numbers_in_a_while_loop_condition_and_body() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    While GetValue() > 42\n        DoThing(7)\n    EndWhile\nEndFunction\n",
            MagicNumbers::Loose,
        );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("42")));
    assert!(diagnostics.iter().any(|d| d.message.contains('7')));
}

#[test]
fn flags_a_magic_number_behind_a_negated_call_and_a_non_negation_unary_operator() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = -GetValue(42)\n    Bool b = !GetFlag(42)\nEndFunction\n",
            MagicNumbers::Loose,
        );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|d| d.message.contains("42")));
}

#[test]
fn flags_magic_numbers_nested_in_array_index_cast_and_new_array_expressions() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int[] values)\n    Int x = values[42]\n    Int y = 99 as Int\n    Int[] arr = new Int[7]\nEndFunction\n",
            MagicNumbers::Loose,
        );

    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics.iter().any(|d| d.message.contains("42")));
    assert!(diagnostics.iter().any(|d| d.message.contains("99")));
    assert!(diagnostics.iter().any(|d| d.message.contains('7')));
}

#[test]
fn flags_a_magic_number_passed_as_a_named_argument() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    DoThing(count = 42)\nEndFunction\n",
        MagicNumbers::Loose,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("42"));
}

#[test]
fn flags_a_non_ignored_float_literal() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    DoThing(3.5)\nEndFunction\n",
        MagicNumbers::Loose,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("3.5"));
}
