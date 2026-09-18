use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check(ast.as_ref())
}

#[test]
fn flags_int_division_in_float_declaration() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Float f = 1 / 2\nEndFunction\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("'f'"));
}

#[test]
fn does_not_flag_constant_int_division_that_divides_evenly() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Float a = 72 / 8\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_negative_constant_int_division_that_divides_evenly() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Float a = -72 / 8\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_folded_constant_division_that_divides_evenly() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Float a = (10 + 2) / (2 * 3)\nEndFunction\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_folded_constant_division_that_truncates() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Float a = (10 - 1) / (2 * 2)\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_literal_division_by_zero_in_float_context() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Float a = 10 / 0\nEndFunction\n");
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn still_flags_constant_int_division_that_truncates() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Float a = 72 / 7\nEndFunction\n");
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn still_flags_int_division_when_an_operand_is_not_a_constant() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int x)\n    Float a = 72 / x\nEndFunction\n");
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_division_with_a_float_operand() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Float f = 1.0 / 2\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_when_one_operand_is_explicitly_cast_to_float() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Float f = (1 as Float) / 2\nEndFunction\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn still_flags_when_the_whole_division_is_cast_to_float() {
    // Casting the *result* of an Int/Int division doesn't undo the
    // truncation that already happened; only casting an operand
    // beforehand does.
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Float f = (1 / 2) as Float\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_plain_int_widening_with_no_division() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int a = 5\n    Float f = a\nEndFunction\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_int_division_assigned_to_an_int_target() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int i = 1 / 2\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_modulo_between_two_ints() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Float f = 5 % 2\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_int_division_assigned_to_a_float_variable() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Float f = 0.0\n    f = 1 / 2\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert!(diagnostics[0].message.contains("variable 'f'"));
}

#[test]
fn flags_compound_assignment_that_widens() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Float f = 0.0\n    f += 1 / 2\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn flags_int_division_returned_from_float_function() {
    let diagnostics =
        check("ScriptName Example\n\nFloat Function Test()\n    Return 1 / 2\nEndFunction\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Test'"));
}

#[test]
fn does_not_flag_return_of_a_float_operand_division() {
    let diagnostics =
        check("ScriptName Example\n\nFloat Function Test()\n    Return 1.0 / 2\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_int_division_passed_to_float_parameter_of_local_function() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Float amount)\nEndFunction\n\nFunction Test()\n    Add(1 / 2)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'amount'"));
    assert!(diagnostics[0].message.contains("'Add'"));
}

#[test]
fn flags_int_division_passed_via_named_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Float first, Float amount)\nEndFunction\n\nFunction Test()\n    Add(1.0, amount = 1 / 2)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'amount'"));
}

#[test]
fn resolves_function_and_named_parameter_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Float Amount)\nEndFunction\n\nFunction Test()\n    add(amount = 1 / 2)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Amount'"));
    assert!(diagnostics[0].message.contains("'Add'"));
}

#[test]
fn flags_nested_local_call_without_attributing_division_to_outer_declaration() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Function Scale(Float amount)\n    Return amount\nEndFunction\n\nFunction Test()\n    Float result = Scale(1 / 2)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("parameter 'amount'"));
    assert!(!diagnostics[0].message.contains("variable 'result'"));
}

#[test]
fn does_not_flag_int_division_when_param_is_int() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Int amount)\nEndFunction\n\nFunction Test()\n    Add(1 / 2)\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_int_division_passed_via_self_qualified_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Float amount)\nEndFunction\n\nFunction Test()\n    self.Add(1 / 2)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'amount'"));
}

#[test]
fn flags_int_division_passed_to_state_function() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Add(1 / 2)\nEndFunction\n\nState Active\n    Function Add(Float amount)\n    EndFunction\nEndState\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'amount'"));
}

#[test]
fn flags_int_divisions_in_state_function_control_flow() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Float Function Ratio(Int value)\n        If value > 0\n            Return value / 2\n        Else\n            While value < 0\n                Float result = value / 3\n                Return result\n            EndWhile\n        EndIf\n    EndFunction\nEndState\n",
        );
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 6);
    assert_eq!(diagnostics[1].line, 9);
}

#[test]
fn flags_assignment_to_float_array_element() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Float[] values)\n    values[0] = 1 / 2\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("array element"));
}

#[test]
fn flags_each_int_division_in_one_expression_separately() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Float f = 1 / 2 + 3 / 4\nEndFunction\n");
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|d| d.line == 4));
}

#[test]
fn flags_int_division_in_script_level_variable_and_property() {
    let diagnostics =
        check("ScriptName Example\n\nFloat _ratio = 1 / 2\nFloat Property Total = 3 / 4 Auto\n");
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("'_ratio'")));
    assert!(diagnostics.iter().any(|d| d.message.contains("'Total'")));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
    assert!(diagnostics.is_empty());
}
