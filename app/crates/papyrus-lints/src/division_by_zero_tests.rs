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
fn flags_division_by_integer_zero_literal() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int a)\n    Int b = a / 0\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains('/'));
}

#[test]
fn flags_modulo_by_float_zero_literal() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Float a)\n    Float b = a % 0.0\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains('%'));
}

#[test]
fn flags_division_by_a_negated_zero_literal() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int a)\n    Int b = a / -0\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_division_by_a_constant_expression_that_folds_to_zero() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int a)\n    Int b = a / (1 - 1)\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_division_by_a_nonzero_literal() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int a)\n    Int b = a / 2\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_division_by_a_runtime_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Int b)\n    Int c = a / b\n    Int d = a / GetValue()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_conditions_return_values_and_nested_state_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Int a)\n        If a / 0 == 1\n        EndIf\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn reports_each_nested_zero_divisor_in_one_statement() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a)\n    Int value = (a / 0) + (a % (2 - 2))\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains('/'));
    assert!(diagnostics[1].message.contains('%'));
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.line == 4));
}

#[test]
fn checks_call_arguments_array_indexes_and_assignment_targets() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Int[] values)\n    Consume(a / 0)\n    Int value = values[a % 0]\n    values[a / 0] = 1\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        vec![4, 5, 6]
    );
}

#[test]
fn constant_folding_handles_mixed_numeric_arithmetic() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Float a)\n    Float value = a / (2 * 0.5 - 1.0)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn flags_division_by_a_folded_division_or_modulo_zero() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a)\n    Int first = a / (0 / 1)\n    Int second = a / (0 % 1)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        vec![4, 5]
    );
}

#[test]
fn still_flags_an_inner_division_by_zero_when_the_outer_divisor_does_not_fold() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    Int value = a / (1 / 0)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains('/'));
}
