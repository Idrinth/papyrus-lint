use super::*;
use crate::config::Config;
use crate::external_signatures::NoExternalSignatures;
use crate::Game;

fn check_with(source: &str, config: &Config) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        config,
        &mut NoExternalSignatures,
    )
}

fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &Config::default())
}

#[test]
fn flags_register_for_update_inside_on_update() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnUpdate()\n    RegisterForUpdate(1.0)\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("RegisterForUpdate"));
    assert!(diagnostics[0].message.contains("RegisterForSingleUpdate"));
}

#[test]
fn flags_register_for_update_game_time_inside_on_update_game_time() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnUpdateGameTime()\n    RegisterForUpdateGameTime(1.0)\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("RegisterForUpdateGameTime"));
}

#[test]
fn flags_register_for_update_inside_on_update_game_time() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnUpdateGameTime()\n    RegisterForUpdate(1.0)\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_qualified_call_on_arbitrary_receiver() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnUpdate()\n    akRef.RegisterForUpdate(1.0)\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_register_for_single_update_inside_on_update() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnUpdate()\n    RegisterForSingleUpdate(1.0)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_register_for_single_update_game_time() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnUpdateGameTime()\n    RegisterForSingleUpdateGameTime(1.0)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_register_for_update_outside_on_update() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(1.0)\nEndFunction\n\nEvent OnUpdate()\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_register_for_update_inside_on_init() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnInit()\n    RegisterForUpdate(1.0)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn matches_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent onupdate()\n    registerforupdate(1.0)\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_inside_named_state_on_update() {
    let diagnostics = check(
        "ScriptName Example\n\nState Busy\n    Event OnUpdate()\n        RegisterForUpdate(0.5)\n    EndEvent\nEndState\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn flags_nested_inside_if_in_on_update() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnUpdate()\n    If true\n        RegisterForUpdate(1.0)\n    EndIf\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn respects_disable_comment() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnUpdate()\n    RegisterForUpdate(1.0) ; @disable register-for-update-in-on-update\nEndEvent\n",
    );
    // Direct check() does not filter disable comments; lint() does.
    assert_eq!(diagnostics.len(), 1);
    let filtered = crate::lint(
        "ScriptName Example\n\nEvent OnUpdate()\n    RegisterForUpdate(1.0) ; @disable register-for-update-in-on-update\nEndEvent\n",
        &Config::default(),
    );
    assert!(filtered.iter().all(|d| d.rule != RULE));
}

#[test]
fn lint_runs_for_skyrim_by_default() {
    let diagnostics = crate::lint(
        "ScriptName Example\n\nEvent OnUpdate()\n    RegisterForUpdate(1.0)\nEndEvent\n",
        &Config::default(),
    );
    assert!(diagnostics.iter().any(|d| d.rule == RULE));
}

#[test]
fn lint_skips_when_game_is_fallout4() {
    let config = Config {
        game: Game::Fallout4,
        ..Default::default()
    };
    let diagnostics = crate::lint(
        "ScriptName Example\n\nEvent OnUpdate()\n    RegisterForUpdate(1.0)\nEndEvent\n",
        &config,
    );
    assert!(diagnostics.iter().all(|d| d.rule != RULE));
}

#[test]
fn lint_skips_when_game_is_starfield() {
    let config = Config {
        game: Game::Starfield,
        ..Default::default()
    };
    let diagnostics = crate::lint(
        "ScriptName Example\n\nEvent OnUpdate()\n    RegisterForUpdate(1.0)\nEndEvent\n",
        &config,
    );
    assert!(diagnostics.iter().all(|d| d.rule != RULE));
}
