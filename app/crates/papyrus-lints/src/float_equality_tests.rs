use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::argument_types::NoExternalSignatures,
    )
}

#[test]
fn flags_float_variables_compared_with_equality() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Float a, Float b)\n    If a == b\n    EndIf\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.starts_with("[info]"));
    assert!(diagnostics[0].message.contains("Float"));
}

#[test]
fn flags_float_variables_compared_with_inequality() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Float a, Float b)\n    If a != b\n    EndIf\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_float_literal_compared_to_float_variable() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Float a)\n    If a == 1.0\n    EndIf\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_ordering_comparisons() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Float a, Float b)\n    If a > b\n    EndIf\n    If a < b\n    EndIf\n    If a >= b\n    EndIf\n    If a <= b\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_int_to_int_comparisons() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Int b)\n    If a == b\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_mismatched_int_and_float_comparisons() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Float b)\n    If a == b\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_comparisons_whose_type_cannot_be_resolved_locally() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Float a)\n    If a == GetValue()\n    EndIf\n    If a == Self.SomeProperty\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn checks_nested_and_state_function_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Float a, Float b)\n        If a > 0.0\n            If a == b\n            EndIf\n        EndIf\n    EndFunction\nEndState\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
    assert!(diagnostics.is_empty());
}
