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
fn flags_int_variable_compared_to_float_literal() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1.0\n    EndIf\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Int and Float"));
}

#[test]
fn flags_float_variable_compared_to_int_variable_with_every_comparison_operator() {
    let diagnostics = check(
        r#"
ScriptName Example

Function Test(Int a, Float f)
    If a == f
    EndIf
    If a != f
    EndIf
    If a < f
    EndIf
    If a <= f
    EndIf
    If a > f
    EndIf
    If a >= f
    EndIf
EndFunction
"#,
    );
    assert_eq!(diagnostics.len(), 6);
}

#[test]
fn flags_mismatched_comparison_outside_a_condition() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Float f)\n    Bool result = a == f\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_int_to_int_or_float_to_float_comparisons() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Int b, Float c, Float d)\n    If a == b\n    EndIf\n    If c == d\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_comparison_with_an_explicit_cast() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Float f)\n    If a == f as Int\n    EndIf\n    If (a as Float) == f\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_comparisons_whose_type_cannot_be_resolved_locally() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a)\n    If a == GetValue()\n    EndIf\n    If a == Self.SomeProperty\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn checks_nested_and_state_function_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Int a, Float f)\n        If a > 0\n            If a == f\n            EndIf\n        EndIf\n    EndFunction\nEndState\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
    assert!(diagnostics.is_empty());
}
