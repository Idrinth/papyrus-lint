use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check(ast.as_ref())
}

#[test]
fn flags_a_direct_assignment_to_an_autoreadonly_property() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    a = 0.2\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("'a'"));
}

#[test]
fn flags_a_self_qualified_assignment() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    Self.a = 0.2\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn flags_a_compound_assignment() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    a += 1.0\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn matches_property_name_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property MyValue = 0.1 AutoReadOnly\n\nFunction Test()\n    myvalue = 0.2\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_plain_auto_property() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a Auto\n\nFunction Test()\n    a = 0.2\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_property_with_no_matching_write() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    Float b = a\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_local_variable_shadowing_the_property_name() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    Float a = 0.2\n    a = 0.3\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_parameter_shadowing_the_property_name() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test(Float a)\n    a = 0.2\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn still_flags_a_self_qualified_write_even_when_shadowed_by_a_local() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    Float a = 0.2\n    Self.a = 0.3\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn flags_assignment_inside_if_block() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    If true\n        a = 0.2\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn flags_assignment_inside_while_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    While true\n        a = 0.2\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nState Active\n    Function Test()\n        a = 0.2\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_member_access_on_an_unrelated_object() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test(SomeQuest quest)\n    quest.a = 0.2\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn returns_no_diagnostics_when_no_property_is_autoreadonly() {
    let diagnostics = check(
            "ScriptName Example\n\nFloat Property a Auto\n\nFunction Test()\n    Int i = 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}
