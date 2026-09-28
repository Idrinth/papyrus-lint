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
fn flags_identical_if_and_elseif_bodies() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Bool a, Bool b)\n    If a\n        Debug.Trace(\"same\")\n    ElseIf b\n        Debug.Trace(\"same\")\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("[info]"));
    assert!(diagnostics[0].message.contains("line 4"));
}

#[test]
fn flags_consecutive_identical_elseif_bodies() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x == 1\n        Return 1\n    ElseIf x == 2\n        Return 2\n    ElseIf x == 3\n        Return 2\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 8);
    assert!(diagnostics[0].message.contains("line 6"));
}

#[test]
fn flags_each_consecutive_pair_in_a_run_of_identical_bodies() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x == 1\n        Return 0\n    ElseIf x == 2\n        Return 0\n    ElseIf x == 3\n        Return 0\n    EndIf\nEndFunction\n",
    );

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        [6, 8]
    );
}

#[test]
fn does_not_flag_different_bodies() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Bool a, Bool b)\n    If a\n        Debug.Trace(\"a\")\n    ElseIf b\n        Debug.Trace(\"b\")\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_non_adjacent_identical_bodies() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int x)\n    If x == 1\n        Return 1\n    ElseIf x == 2\n        Return 2\n    ElseIf x == 3\n        Return 1\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_compare_against_else_body() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Bool a)\n    If a\n        Debug.Trace(\"same\")\n    Else\n        Debug.Trace(\"same\")\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_empty_bodies() {
    // empty-body owns empty branches; identical empty ones stay silent here.
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Bool a, Bool b)\n    If a\n    ElseIf b\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_line_differences_when_comparing_bodies() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Bool a, Bool b)\n    If a\n        Int x = 1\n        x += 1\n    ElseIf b\n        Int x = 1\n        x += 1\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn checks_nested_if_statements() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Bool outer, Bool a, Bool b)\n    If outer\n        If a\n            Return 1\n        ElseIf b\n            Return 1\n        EndIf\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn checks_state_function_bodies() {
    let diagnostics = check(
        "ScriptName Example\n\nState Active\n    Function Test(Bool a, Bool b)\n        If a\n            Debug.Trace(\"same\")\n        ElseIf b\n            Debug.Trace(\"same\")\n        EndIf\n    EndFunction\nEndState\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn disable_directives_suppress_diagnostics() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\nFunction Test(Bool a, Bool b)\n    If a\n        Debug.Trace(\"same\")\n    ElseIf b ; @disable identical-if-bodies\n        Debug.Trace(\"same\")\n    EndIf\nEndFunction\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file identical-if-bodies\nScriptName Example\n\nFunction Test(Bool a, Bool b)\n    If a\n        Debug.Trace(\"same\")\n    ElseIf b\n        Debug.Trace(\"same\")\n    EndIf\nEndFunction\n",
        &crate::config::Config::default(),
    );

    assert!(line_disabled
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
    assert!(file_disabled
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_off_switch_suppresses_diagnostics() {
    let source = "ScriptName Example\n\nFunction Test(Bool a, Bool b)\n    If a\n        Debug.Trace(\"same\")\n    ElseIf b\n        Debug.Trace(\"same\")\n    EndIf\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.identical_if_bodies = false;

    let diagnostics = crate::lint(source, &config);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}
