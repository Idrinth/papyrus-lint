use super::*;

fn check(source: &str, allow_bool_like_int: bool) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check(ast.as_ref(), allow_bool_like_int)
}

#[test]
fn flags_non_boolean_if_and_while_conditions() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int count)\n    If count\n    EndIf\n    While count\n    EndWhile\nEndFunction\n",
            true,
        );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (4, 5));
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Int"));
    assert_eq!((diagnostics[1].line, diagnostics[1].column), (6, 5));
}

#[test]
fn flags_each_elseif_branch_independently() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Bool b)\n    If b\n    ElseIf a\n    Else\n    EndIf\nEndFunction\n",
            true,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn allows_boolean_literals_comparisons_and_logical_expressions() {
    let diagnostics = check(
        r#"
ScriptName Example

Function Test(Int a, Bool flag)
    If true
    EndIf
    If flag
    EndIf
    If a > 0
    EndIf
    If flag && a > 0
    EndIf
    If !flag
    EndIf
    While a > 0
    EndWhile
EndFunction
"#,
        true,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_conditions_that_cannot_be_resolved_locally() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If GetValue()\n    EndIf\n    If Self.SomeProperty\n    EndIf\nEndFunction\n",
            true,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_nested_and_state_function_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Int a)\n        If a > 0\n            If a\n            EndIf\n        EndIf\n    EndFunction\nEndState\n",
            true,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn reports_the_concrete_type_for_each_non_boolean_condition() {
    let diagnostics = check(
        r#"ScriptName Example

Function Test(String text, Float ratio, Form target)
    If text
    EndIf
    If ratio
    EndIf
    If target
    EndIf
EndFunction
"#,
        true,
    );

    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics[0].message.contains("found 'String'"));
    assert!(diagnostics[1].message.contains("found 'Float'"));
    assert!(diagnostics[2].message.contains("found 'Form'"));
}

#[test]
fn reports_array_conditions_with_array_notation() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int[] values)\n    If values\n    EndIf\nEndFunction\n",
            true,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("found 'Int[]'"));
}

#[test]
fn resolves_script_properties_and_function_locals() {
    let diagnostics = check(
        r#"ScriptName Example

Int Property Count Auto

Function Test()
    String label = "ready"
    If Count
    EndIf
    While label
    EndWhile
EndFunction
"#,
        true,
    );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 7);
    assert!(diagnostics[0].message.contains("found 'Int'"));
    assert_eq!(diagnostics[1].line, 9);
    assert!(diagnostics[1].message.contains("found 'String'"));
}

#[test]
fn function_parameters_do_not_leak_into_the_next_function() {
    let diagnostics = check(
        r#"ScriptName Example

Function First(Int value)
EndFunction

Function Second()
    If value
    EndIf
EndFunction
"#,
        true,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_conditions_inside_else_bodies() {
    let diagnostics = check(
        r#"ScriptName Example

Function Test(Bool ready, Int attempts)
    If ready
    Else
        While attempts
        EndWhile
    EndIf
EndFunction
"#,
        true,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (6, 9));
}

#[test]
fn invalid_source_returns_no_diagnostics() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int count)\n    If count\nEndFunction\n",
        true,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn allows_bool_like_int_literals_by_default() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If 1\n    EndIf\n    While 0\n    EndWhile\nEndFunction\n",
            true,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_bool_like_int_literals_when_disallowed() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If 1\n    EndIf\n    While 0\n    EndWhile\nEndFunction\n",
            false,
        );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("found 'Int'"));
    assert!(diagnostics[1].message.contains("found 'Int'"));
}

#[test]
fn still_flags_int_literals_other_than_one_and_zero() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    If 2\n    EndIf\nEndFunction\n",
        true,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("found 'Int'"));
}

#[test]
fn still_flags_int_variables_holding_zero_or_one() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int count)\n    If count\n    EndIf\nEndFunction\n",
        true,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("found 'Int'"));
}
