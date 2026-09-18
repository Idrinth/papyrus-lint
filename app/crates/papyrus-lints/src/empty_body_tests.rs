use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(ast.as_ref(), tokens.as_deref())
}

#[test]
fn flags_completely_empty_while_loop() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    While true\n    EndWhile\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("Loop body is empty"));
}

#[test]
fn flags_while_loop_that_only_increments_its_counter() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 0\n    While i < 10\n        i += 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert!(diagnostics[0].message.contains("increments or decrements"));
}

#[test]
fn flags_while_loop_that_only_decrements_its_counter() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 10\n    While i > 0\n        i -= 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("increments or decrements"));
}

#[test]
fn flags_while_loop_using_plain_assignment_increment() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 0\n    While i < 10\n        i = i + 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);

    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 10\n    While i > 0\n        i = i - 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_loop_with_actual_content() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 0\n    While i < 10\n        i += 1\n        Debug.Trace(\"tick\")\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_step_built_from_a_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 0\n    While i < 10\n        i += GetStep()\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_multiplicative_step() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 1\n    While i < 100\n        i *= 2\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_empty_if_body() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("Empty If/ElseIf body"));
}

#[test]
fn flags_empty_elseif_body() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool a, Bool b)\n    If a\n        Debug.Trace(\"a\")\n    ElseIf b\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn flags_empty_else_body() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n        Debug.Trace(\"a\")\n    Else\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
    assert!(diagnostics[0].message.contains("Empty Else body"));
}

#[test]
fn does_not_flag_an_if_with_no_else_clause_at_all() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n        Debug.Trace(\"a\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_non_empty_else_body() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n        Debug.Trace(\"a\")\n    Else\n        Debug.Trace(\"b\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_nested_and_state_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Bool flag)\n        If flag\n            While true\n            EndWhile\n        EndIf\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn empty_else_check_still_runs_on_unparseable_source() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(\n    If flag\n    Else\n    EndIf\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Empty Else body"));
}
