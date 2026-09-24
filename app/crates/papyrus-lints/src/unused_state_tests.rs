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
fn flags_a_non_auto_state_without_a_goto_state_call() {
    let diagnostics = check("ScriptName Example\n\nState Waiting\nEndState\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("'Waiting'"));
}

#[test]
fn ignores_an_auto_state() {
    assert!(check("ScriptName Example\n\nAuto State Waiting\nEndState\n").is_empty());
}

#[test]
fn ignores_a_state_targeted_by_a_goto_state_call_case_insensitively() {
    let source = "ScriptName Example\n\nState Waiting\nEndState\n\nFunction Start()\n    GoToState(\"waiting\")\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn recognizes_a_self_qualified_goto_state_call() {
    let source = "ScriptName Example\n\nState Waiting\nEndState\n\nFunction Start()\n    self.GoToState(\"Waiting\")\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn reports_only_states_without_a_matching_target() {
    let source = "ScriptName Example\n\nState Waiting\nEndState\n\nState Running\nEndState\n\nFunction Start()\n    GoToState(\"Running\")\nEndFunction\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Waiting'"));
}

#[test]
fn does_not_treat_another_objects_call_as_a_use() {
    let source = "ScriptName Example\n\nState Waiting\nEndState\n\nFunction Start(Example other)\n    other.GoToState(\"Waiting\")\nEndFunction\n";

    assert_eq!(check(source).len(), 1);
}

#[test]
fn does_not_guess_the_target_of_a_non_literal_call() {
    let source = "ScriptName Example\n\nState Waiting\nEndState\n\nFunction Start(String target)\n    GoToState(target)\nEndFunction\n";

    assert_eq!(check(source).len(), 1);
}

#[test]
fn honors_line_and_file_disables() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\nState Waiting ; @disable unused-state\nEndState\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file unused-state\nScriptName Example\n\nState Waiting\nEndState\n",
        &crate::config::Config::default(),
    );

    assert!(line_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
    assert!(file_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn honors_the_config_off_switch() {
    let mut config = crate::config::Config::default();
    config.rules.unused_state = false;

    let diagnostics = crate::lint("ScriptName Example\n\nState Waiting\nEndState\n", &config);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn returns_no_diagnostics_without_an_ast() {
    assert!(check("ScriptName Example\n\nState").is_empty());
}
