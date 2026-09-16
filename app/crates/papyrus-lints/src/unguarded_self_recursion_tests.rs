use super::*;

#[test]
fn flags_a_function_that_unconditionally_calls_itself() {
    let source = "ScriptName Example\n\nFunction Foo()\n    Foo()\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
}

#[test]
fn flags_regardless_of_argument_values_and_surrounding_statements() {
    let source =
            "ScriptName Example\n\nFunction Foo(Int x)\n    Debug.Trace(\"x\")\n    Foo(x - 1)\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn flags_a_self_call_reached_through_self() {
    let source = "ScriptName Example\n\nFunction Foo()\n    Self.Foo()\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_self_call_used_as_a_return_value() {
    let source = "ScriptName Example\n\nInt Function Foo()\n    Return Foo()\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_self_call_nested_in_an_assignments_value() {
    let source =
            "ScriptName Example\n\nInt Function Foo()\n    Int x = 1 + Foo()\n    Return x\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn matches_the_function_name_case_insensitively() {
    let source = "ScriptName Example\n\nFunction Foo()\n    FOO()\nEndFunction\n";

    assert_eq!(check(source).len(), 1);
}

#[test]
fn does_not_flag_the_classic_if_return_guarded_base_case() {
    let source = "ScriptName Example\n\nFunction Foo(Int x)\n    If x <= 0\n        Return\n    EndIf\n    Foo(x - 1)\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn flags_a_self_call_after_an_if_with_no_return_in_it() {
    let source = "ScriptName Example\n\nFunction Foo(Int x)\n    If x <= 0\n        ; no return happening\n    EndIf\n    Foo(x - 1)\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn does_not_flag_when_only_the_else_branch_returns() {
    let source = "ScriptName Example\n\nFunction Foo(Int x)\n    If x <= 0\n        Debug.Trace(\"base case\")\n    Else\n        Return\n    EndIf\n    Foo(x - 1)\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_when_the_return_is_nested_in_a_further_if() {
    let source = "ScriptName Example\n\nFunction Foo(Int x)\n    If x <= 0\n        If True\n            Return\n        EndIf\n    EndIf\n    Foo(x - 1)\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_recursion_inside_a_while_loop() {
    let source =
            "ScriptName Example\n\nFunction Foo(Int x)\n    While x > 0\n        Foo(x - 1)\n    EndWhile\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_a_call_to_a_different_function() {
    let source =
            "ScriptName Example\n\nFunction Foo()\n    Bar()\nEndFunction\n\nFunction Bar()\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_a_function_with_no_self_call() {
    let source = "ScriptName Example\n\nFunction Foo()\n    Debug.Trace(\"hi\")\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_a_self_call_gated_by_short_circuit_and() {
    let source =
            "ScriptName Example\n\nBool Function Foo(Bool cond)\n    Return cond && Foo(false)\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn flags_a_self_call_on_the_eager_side_of_short_circuit_operators() {
    let source = "ScriptName Example\n\nBool Function Foo(Bool cond)\n    Bool first = Foo(false) && cond\n    Return Foo(false) || cond\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        [4, 5]
    );
}

#[test]
fn does_not_flag_a_self_call_gated_by_short_circuit_or() {
    let source =
            "ScriptName Example\n\nBool Function Foo(Bool cond)\n    Return cond || Self.Foo(false)\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_treat_a_same_named_method_on_another_object_as_a_self_call() {
    let source =
        "ScriptName Example\n\nFunction Foo(Example other)\n    other.Foo(None)\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_a_native_function() {
    let source = "ScriptName Example\n\nFunction Foo() Native\n";

    assert!(check(source).is_empty());
}

#[test]
fn checks_functions_declared_in_states_too() {
    let source =
            "ScriptName Example\n\nState Active\n    Function Foo()\n        Foo()\n    EndFunction\nEndState\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn flags_every_branch_of_an_exhaustive_if_that_all_recurse() {
    let source = "ScriptName Example\n\nFunction A()\n    If z == 1\n        A()\n        B()\n    ElseIf q == 9\n        B()\n        A()\n    Else\n        A()\n    EndIf\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[1].line, 9);
    assert_eq!(diagnostics[2].line, 11);
}

#[test]
fn does_not_flag_an_exhaustive_if_when_one_branch_does_not_recurse() {
    let source = "ScriptName Example\n\nFunction A()\n    If z == 1\n        A()\n    Else\n        B()\n    EndIf\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_an_if_with_no_else_even_when_every_branch_recurses() {
    let source = "ScriptName Example\n\nFunction A()\n    If z == 1\n        A()\n    ElseIf q == 9\n        A()\n    EndIf\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Foo(\nEndFunction\n").is_empty());
}

#[test]
fn does_not_flag_the_gotostate_save_compat_idiom() {
    let source = "ScriptName Example\n\nState Done\n    Event OnActivate(ObjectReference akActivator)\n    EndEvent\nEndState\n\nEvent OnActivate(ObjectReference akActivator)\n    GoToState(\"Done\")\n    OnActivate(akActivator)\nEndEvent\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_gotostate_called_through_self() {
    let source = "ScriptName Example\n\nState Done\n    Event OnActivate(ObjectReference akActivator)\n    EndEvent\nEndState\n\nEvent OnActivate(ObjectReference akActivator)\n    Self.GoToState(\"Done\")\n    OnActivate(akActivator)\nEndEvent\n";

    assert!(check(source).is_empty());
}

#[test]
fn still_flags_when_the_target_state_declares_no_matching_handler() {
    let source = "ScriptName Example\n\nState Done\n    Event OnLoad()\n    EndEvent\nEndState\n\nEvent OnActivate(ObjectReference akActivator)\n    GoToState(\"Done\")\n    OnActivate(akActivator)\nEndEvent\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 10);
}

#[test]
fn still_flags_when_gotostate_has_a_non_literal_target() {
    let source = "ScriptName Example\n\nState Done\n    Function Foo(String targetState)\n    EndFunction\nEndState\n\nFunction Foo(String targetState)\n    GoToState(targetState)\n    Foo(targetState)\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 10);
}

#[test]
fn ignores_gotostate_calls_made_on_another_object() {
    let source = "ScriptName Example\n\nState Done\n    Function Foo(Example other)\n    EndFunction\nEndState\n\nFunction Foo(Example other)\n    other.GoToState(\"Done\")\n    Foo(other)\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 10);
}

#[test]
fn matches_gotostate_targets_and_handlers_case_insensitively() {
    let source = "ScriptName Example\n\nState Done\n    Function Foo()\n    EndFunction\nEndState\n\nFunction Foo()\n    GoToState(\"dOnE\")\n    Foo()\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn still_flags_when_gotostate_targets_the_functions_own_state() {
    let source = "ScriptName Example\n\nState Active\n    Event OnActivate(ObjectReference akActivator)\n        GoToState(\"Active\")\n        OnActivate(akActivator)\n    EndEvent\nEndState\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn still_flags_a_self_call_preceding_a_gotostate_that_would_otherwise_guard_it() {
    let source = "ScriptName Example\n\nState Done\n    Event OnActivate(ObjectReference akActivator)\n    EndEvent\nEndState\n\nEvent OnActivate(ObjectReference akActivator)\n    OnActivate(akActivator)\n    GoToState(\"Done\")\nEndEvent\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 9);
}

#[test]
fn finds_self_calls_nested_in_every_eager_expression_shape() {
    let source = "ScriptName Example\n\nInt Function Foo(Int[] values)\n    Int negated = -Foo(values)\n    Int indexed = values[Foo(values)]\n    Int casted = Foo(values) as Int\n    Int[] sized = new Int[Foo(values)]\n    Consume(value = Foo(values))\n    Return negated + indexed + casted + sized[0]\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        [4, 5, 6, 7, 8]
    );
}

#[test]
fn exhaustive_branches_recognize_nested_self_calls_in_expression_shapes() {
    let source = "ScriptName Example\n\nInt Function Foo(Int[] values, Bool choose)\n    If choose\n        Int first = -Foo(values, choose)\n    Else\n        Consume(value = values[Foo(values, choose)])\n    EndIf\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        [5, 7]
    );
}

#[test]
fn nested_conditional_self_calls_do_not_make_a_branch_always_recurse() {
    let source = "ScriptName Example\n\nFunction Foo(Bool first, Bool second)\n    If first\n        If second\n            Foo(first, second)\n        EndIf\n    Else\n        Foo(first, second)\n    EndIf\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn nested_while_return_is_treated_as_a_possible_guard() {
    let source = "ScriptName Example\n\nFunction Foo(Int x)\n    If x <= 0\n        While x < -1\n            Return\n        EndWhile\n    EndIf\n    Foo(x - 1)\nEndFunction\n";

    assert!(check(source).is_empty());
}
