use super::*;

#[test]
fn flags_a_state_function_with_a_mismatched_parameter_type() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(Int name)\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0]
        .message
        .contains("Parameter 1 of 'Greet' in state 'Loud'"));
    assert!(diagnostics[0].message.contains("is declared Int"));
    assert!(diagnostics[0].message.contains("declares String"));
}

#[test]
fn flags_a_state_function_with_a_mismatched_parameter_count() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(String name, Int volume)\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("Function 'Greet' in state 'Loud'"));
    assert!(diagnostics[0].message.contains("declares 2 parameters"));
    assert!(diagnostics[0]
        .message
        .contains("empty state's declaration declares 1 parameter"));
}

#[test]
fn flags_a_state_function_with_a_mismatched_return_type() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function MyFunction()\n    Return 1\nEndFunction\n\nState MyState\n    String Function MyFunction()\n        Return \"hi\"\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("Function 'MyFunction' in state 'MyState'"));
    assert!(diagnostics[0]
        .message
        .contains("declares return type String"));
    assert!(diagnostics[0]
        .message
        .contains("empty state's declaration declares Int"));
}

#[test]
fn flags_a_state_function_that_gained_a_return_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction MyFunction()\nEndFunction\n\nState MyState\n    Int Function MyFunction()\n        Return 1\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("declares return type Int"));
    assert!(diagnostics[0]
        .message
        .contains("empty state's declaration declares no value"));
}

#[test]
fn allows_a_matching_state_override() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function MyFunction()\n    Return 1\nEndFunction\n\nState MyState\n    Int Function MyFunction()\n        Return 2\n    EndFunction\nEndState\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn allows_a_matching_state_override_with_parameters() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(String name)\n    EndFunction\nEndState\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_state_function_with_no_local_empty_state_counterpart() {
    // This might still be valid via a parent script's empty state,
    // which this lint has no way to resolve; see the module docs.
    let diagnostics = check(
            "ScriptName Example Extends ParentScript\n\nState Loud\n    Function Greet(Int volume)\n    EndFunction\nEndState\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn allows_a_state_override_whose_parameter_type_casing_differs() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(bool flag)\nEndFunction\n\nState Loud\n    Function Greet(Bool flag)\n    EndFunction\nEndState\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn allows_a_state_override_whose_return_type_casing_differs() {
    let diagnostics = check(
            "ScriptName Example\n\nbool Function IsLoud()\n    Return false\nEndFunction\n\nState Loud\n    Bool Function IsLoud()\n        Return true\n    EndFunction\nEndState\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_function_names_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function GREET(Int name)\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_each_state_separately() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(Int name)\n    EndFunction\nEndState\n\nState Quiet\n    Function Greet(Bool name)\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics =
        check("ScriptName Example\n\nState Loud\n    Function Greet(\n    EndFunction\nEndState\n");
    assert!(diagnostics.is_empty());
}
