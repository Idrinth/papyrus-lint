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

fn check_starfield(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse_with_mode(
        source,
        papyrus_parser::parser::GameEdition::Starfield,
    )
    .ok();
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
fn recognizes_float_reversed_addition_and_case_insensitive_identifiers() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Float value = 0.0\n    While value < 10.0\n        VALUE = 0.5 + value\n    EndWhile\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("increments or decrements"));
}

#[test]
fn does_not_flag_non_equivalent_plain_assignments() {
    for assignment in [
        "i = 1 - i",
        "i = i * 2",
        "i = 1",
        "values[i] = values[i] + 1",
    ] {
        let source = format!(
            "ScriptName Example\n\nFunction Test(Int[] values)\n    Int i = 1\n    While i < 10\n        {assignment}\n    EndWhile\nEndFunction\n"
        );

        assert!(check(&source).is_empty(), "flagged assignment: {assignment}");
    }
}

#[test]
fn does_not_flag_other_compound_assignments() {
    for operator in ["*=", "/=", "%="] {
        let source = format!(
            "ScriptName Example\n\nFunction Test()\n    Int i = 2\n    While i < 10\n        i {operator} 2\n    EndWhile\nEndFunction\n"
        );

        assert!(check(&source).is_empty(), "flagged operator: {operator}");
    }
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
fn flags_empty_lock_guard_body() {
    let diagnostics = check_starfield(
        "ScriptName Example\n\nFunction Test()\n    LockGuard MyGuard\n    EndLockGuard\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("LockGuard body is empty"));
}

#[test]
fn flags_empty_try_lock_guard_body_and_else_body() {
    let diagnostics = check_starfield(
        "ScriptName Example\n\nFunction Test()\n    TryLockGuard MyGuard\n    ElseTryLockGuard\n    EndTryLockGuard\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[1].line, 5);
    assert!(diagnostics[1]
        .message
        .contains("Empty ElseTryLockGuard body"));
}

#[test]
fn accepts_non_empty_lock_guard_and_try_lock_guard_bodies() {
    let diagnostics = check_starfield(
        "ScriptName Example\n\nFunction Test()\n    LockGuard MyGuard\n        Debug.Trace(\"locked\")\n    EndLockGuard\n    TryLockGuard MyGuard\n        Debug.Trace(\"acquired\")\n    ElseTryLockGuard\n        Debug.Trace(\"busy\")\n    EndTryLockGuard\nEndFunction\n",
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

#[test]
fn malformed_non_empty_else_is_not_flagged_by_token_fallback() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(\n    If flag\n    Else\n        Debug.Trace(\"fallback\")\n    EndIf\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn token_fallback_accepts_missing_tokens() {
    assert!(empty_else_diagnostics(None).is_empty());
}

#[test]
fn disable_directives_suppress_diagnostics() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\nFunction Test()\n    While true ; @disable empty-body\n    EndWhile\nEndFunction\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file empty-body\nScriptName Example\n\nFunction Test()\n    While true\n    EndWhile\nEndFunction\n",
        &crate::config::Config::default(),
    );

    assert!(line_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
    assert!(file_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_off_switch_suppresses_diagnostics() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    While true\n    EndWhile\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.empty_body = false;

    let diagnostics = crate::lint(source, &config);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}
