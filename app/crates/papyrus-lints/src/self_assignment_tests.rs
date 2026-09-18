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
fn flags_a_local_variable_assigned_to_itself() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int a = 10\n    a = a\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
}

#[test]
fn matches_the_name_case_insensitively() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int a = 10\n    a = A\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_self_qualified_property_assigned_to_itself() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property a Auto\n\nFunction Test()\n    Self.a = Self.a\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_matching_member_chain_assigned_to_itself() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(SomeQuest akQuest)\n    akQuest.Stage = akQuest.Stage\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_bare_name_assigned_to_a_self_qualified_one() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property a Auto\n\nFunction Test()\n    a = Self.a\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_compound_assignment() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int a = 10\n    a += a\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_assignment_of_a_different_variable() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 10\n    Int b = 5\n    a = b\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_call_even_when_written_identically_on_both_sides() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(SomeQuest akQuest)\n    Int a = akQuest.GetStage()\n    a = akQuest.GetStage()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_index_expression() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[3]\n    a[0] = a[0]\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_assignment_inside_if_block() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 10\n    If true\n        a = a\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn flags_assignment_inside_while_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 10\n    While a > 0\n        a = a\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Waiting\n    Function Test()\n        Int a = 10\n        a = a\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn returns_no_diagnostics_for_a_script_that_fails_to_parse() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\n    a = a\nEndFunction\n");

    assert!(diagnostics.is_empty());
}
