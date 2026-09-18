use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check(ast.as_ref())
}

#[test]
fn flags_a_function_with_a_single_statement() {
    let diagnostics = check("ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("'A'"));
}

#[test]
fn does_not_flag_an_event_with_a_single_statement() {
    let diagnostics = check("ScriptName Example\n\nEvent OnInit()\n    B()\nEndEvent\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_fragment_function_with_a_single_statement() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Fragment_0(ObjectReference akSpeakerRef)\n    B()\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_fragment_function_regardless_of_case() {
    let diagnostics = check("ScriptName Example\n\nFunction fragment_12()\n    B()\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_function_with_no_statements() {
    let diagnostics = check("ScriptName Example\n\nFunction A()\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_function_with_more_than_one_statement() {
    let diagnostics = check("ScriptName Example\n\nFunction A()\n    B()\n    C()\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn a_single_if_statement_still_counts_as_one_statement_even_when_nested_is_bigger() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction A()\n    If ready\n        B()\n        C()\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function A()\n        B()\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'A'"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction A(\nEndFunction\n").is_empty());
}

#[test]
fn repair_inlines_call_sites_of_a_zero_argument_wrapper() {
    let source = "ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n\nFunction Caller()\n    A()\nEndFunction\n";

    let repaired = repair(source);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n\nFunction Caller()\n    B()\nEndFunction\n"
        );
    // The wrapper's own declaration is left in place, so it's still
    // flagged (as is `Caller`, now itself a single-statement wrapper).
    assert_eq!(check(&repaired).len(), 2);
}

#[test]
fn repair_inlines_call_sites_of_a_pass_through_wrapper() {
    let source = "ScriptName Example\n\nFunction A(Int x, Bool y)\n    B(x, y)\nEndFunction\n\nFunction Caller()\n    A(1, true)\nEndFunction\n";

    let repaired = repair(source);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction A(Int x, Bool y)\n    B(x, y)\nEndFunction\n\nFunction Caller()\n    B(1, true)\nEndFunction\n"
        );
}

#[test]
fn repair_rewrites_a_self_qualified_call_site_to_a_self_qualified_wrapped_call() {
    let source = "ScriptName Example\n\nFunction A()\n    Self.B()\nEndFunction\n\nFunction Caller()\n    Self.A()\nEndFunction\n";

    let repaired = repair(source);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction A()\n    Self.B()\nEndFunction\n\nFunction Caller()\n    Self.B()\nEndFunction\n"
        );
}

#[test]
fn repair_rewrites_a_call_nested_inside_another_expression() {
    let source = "ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n\nFunction Caller(Bool cond)\n    If cond && A()\n        C(A())\n    EndIf\nEndFunction\n";

    let repaired = repair(source);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n\nFunction Caller(Bool cond)\n    If cond && B()\n        C(B())\n    EndIf\nEndFunction\n"
        );
}

#[test]
fn repair_leaves_a_wrapper_whose_arguments_are_not_a_pure_forward_untouched() {
    let source = "ScriptName Example\n\nFunction A(Int x, Int y)\n    B(y, x)\nEndFunction\n\nFunction Caller()\n    A(1, 2)\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_a_wrapper_with_a_defaulted_parameter_untouched() {
    let source = "ScriptName Example\n\nFunction A(Int x = 1)\n    B(x)\nEndFunction\n\nFunction Caller()\n    A()\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_a_call_site_using_a_named_argument_untouched() {
    let source = "ScriptName Example\n\nFunction A(Int x)\n    B(x)\nEndFunction\n\nFunction Caller()\n    A(x = 1)\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_a_wrapper_alone_when_its_name_is_shared_by_a_state_override() {
    let source = "ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n\nState Active\n    Function A()\n        C()\n    EndFunction\nEndState\n\nFunction Caller()\n    A()\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_a_function_whose_single_statement_is_not_a_call_untouched() {
    let source = "ScriptName Example\n\nFunction A()\n    Int x = 1\nEndFunction\n\nFunction Caller()\n    A()\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_a_call_to_parent_untouched() {
    let source = "ScriptName Example\n\nFunction A()\n    Parent.A()\nEndFunction\n\nFunction Caller()\n    A()\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_a_fragment_function_wrapper_untouched() {
    let source = "ScriptName Example\n\nFunction Fragment_0(ObjectReference akSpeakerRef)\n    B()\nEndFunction\n\nFunction Caller()\n    Fragment_0(None)\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_is_a_no_op_when_no_wrapper_qualifies() {
    let source = "ScriptName Example\n\nFunction A()\n    B()\n    C()\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn does_not_crash_repairing_unparseable_source() {
    let source = "ScriptName Example\n\nFunction A(\nEndFunction\n";
    assert_eq!(repair(source), source);
}
