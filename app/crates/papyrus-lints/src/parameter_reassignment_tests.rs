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
fn flags_direct_parameter_reassignment() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int total)\n    total = 1\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'total'"));
}

#[test]
fn flags_compound_parameter_reassignment() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int total)\n    total += 1\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn matches_parameter_name_case_insensitively() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int total)\n    TOTAL = 1\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_local_variable_reassignment() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int total)\n    Int other = 0\n    other = 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_member_or_index_assignment_built_from_a_parameter() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    akRef.Foo = 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_parameter_reassignment_inside_if_block() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int total)\n    If true\n        total = 1\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn flags_parameter_reassignment_inside_while_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int total)\n    While total > 0\n        total -= 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Waiting\n    Function Test(Int total)\n        total = 1\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_a_function_with_no_parameters() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int total = 0\n    total = 1\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn returns_no_diagnostics_for_a_script_that_fails_to_parse() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int total\n    total = 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}
