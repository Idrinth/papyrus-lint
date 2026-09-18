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
fn flags_random_int_with_reversed_bounds() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = Utility.RandomInt(10, 5)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("RandomInt(10, 5)"));
}

#[test]
fn flags_random_float_with_equal_bounds() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = Utility.RandomFloat(1.0, 1.0)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("RandomFloat(1, 1)"));
}

#[test]
fn does_not_flag_random_int_with_correct_bounds() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = Utility.RandomInt(0, 10)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_random_float_with_correct_bounds() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = Utility.RandomFloat(0.0, 1.0)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_unqualified_call() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int i = RandomInt(10, 5)\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_call_on_an_unrelated_receiver() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(MyScript akOther)\n    Int i = akOther.RandomInt(10, 5)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_named_arguments() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = Utility.RandomInt(akMin = 10, akMax = 5)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_runtime_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int aiMin, Int aiMax)\n    Int i = Utility.RandomInt(aiMin, aiMax)\n    Int j = Utility.RandomInt(GetMin(), 10)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_conditions_return_values_and_nested_state_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        If true\n            Int i = Utility.RandomInt(10, 5)\n        EndIf\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn does_not_crash_on_a_call_with_missing_arguments() {
    assert!(check(
        "ScriptName Example\n\nFunction Test()\n    Int i = Utility.RandomInt(10)\nEndFunction\n"
    )
    .is_empty());
}

#[test]
fn walks_extra_arguments_beyond_the_bounds() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = Utility.RandomInt(10, 5, GetValue())\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_constant_expression_that_folds_to_reversed_bounds() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = Utility.RandomInt(2 + 3, 4 * 1)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("RandomInt(5, 4)"));
}

#[test]
fn does_not_flag_a_call_whose_callee_is_neither_an_identifier_nor_a_member_access() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Self(10, 5)\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn as_number_rejects_a_non_numeric_literal() {
    assert_eq!(as_number(&Literal::Bool(true)), None);
}
