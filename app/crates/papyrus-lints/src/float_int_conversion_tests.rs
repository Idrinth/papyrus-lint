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
fn flags_float_literal_in_int_declaration() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int x = 1.5\nEndFunction\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("'x'"));
}

#[test]
fn flags_float_variable_assigned_to_int_variable() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = 1.5\n    Int x = 0\n    x = f\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
    assert!(diagnostics[0].message.contains("variable 'x'"));
}

#[test]
fn flags_compound_assignment_that_narrows() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = 0.5\n    Int x = 1\n    x += f\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_flag_explicit_cast_to_int() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = 1.5\n    Int x = f as Int\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_int_to_int_or_float_to_float() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 1\n    Int b = a\n    Float c = 1.5\n    Float d = c\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_int_widening_to_float() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int a = 1\n    Float f = a\nEndFunction\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_float_returned_from_int_function() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Function Test()\n    Float f = 1.5\n    Return f\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Test"));
}

#[test]
fn does_not_flag_explicit_cast_in_return() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test()\n    Float f = 1.5\n    Return f as Int\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_float_argument_passed_to_int_parameter_of_local_function() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Int amount)\nEndFunction\n\nFunction Test()\n    Float f = 1.5\n    Add(f)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'amount'"));
    assert!(diagnostics[0].message.contains("'Add'"));
}

#[test]
fn flags_float_argument_passed_via_named_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Int first, Int amount)\nEndFunction\n\nFunction Test()\n    Float f = 1.5\n    Add(1, amount = f)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'amount'"));
    assert!(diagnostics[0].message.contains("'Add'"));
}

#[test]
fn does_not_flag_float_argument_when_param_is_float() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Float amount)\nEndFunction\n\nFunction Test()\n    Float f = 1.5\n    Add(f)\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_narrowing_nested_inside_a_call_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Add(Int amount)\n    Return 0\nEndFunction\n\nFunction Outer(Float amount)\nEndFunction\n\nFunction Test()\n    Float f = 1.5\n    Outer(Add(f))\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Add'"));
}

#[test]
fn flags_float_literal_in_script_level_variable_and_property() {
    let diagnostics =
        check("ScriptName Example\n\nInt _count = 1.5\nInt Property Total = 2.5 Auto\n");
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("'_count'")));
    assert!(diagnostics.iter().any(|d| d.message.contains("'Total'")));
}

#[test]
fn flags_float_argument_passed_via_self_qualified_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Int amount)\nEndFunction\n\nFunction Test()\n    Float f = 1.5\n    self.Add(f)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'amount'"));
}

#[test]
fn flags_float_argument_passed_to_state_function() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = 1.5\n    Add(f)\nEndFunction\n\nState Active\n    Function Add(Int amount)\n    EndFunction\nEndState\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'amount'"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
    assert!(diagnostics.is_empty());
}
