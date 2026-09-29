use super::*;
use crate::external_signatures::NoExternalSignatures;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut NoExternalSignatures,
    )
}

const DEAD_ONINIT: &str = "ScriptName Example\n\n\
State Busy\n\
  Event OnInit()\n\
    Setup()\n\
  EndEvent\n\
EndState\n";

#[test]
fn flags_oninit_inside_non_auto_state() {
    let diagnostics = check(DEAD_ONINIT);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Busy"));
    assert!(diagnostics[0].message.contains("OnBeginState"));
}

#[test]
fn does_not_flag_oninit_inside_auto_state() {
    let source = "ScriptName Example\n\n\
Auto State Busy\n\
  Event OnInit()\n\
    Setup()\n\
  EndEvent\n\
EndState\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_empty_state_oninit_with_onbeginstate() {
    let source = "ScriptName Example\n\n\
Event OnInit()\n\
  GoToState(\"Busy\")\n\
EndEvent\n\n\
State Busy\n\
  Event OnBeginState()\n\
    Setup()\n\
  EndEvent\n\
EndState\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_other_events_inside_non_auto_state() {
    let source = "ScriptName Example\n\n\
State Busy\n\
  Event OnUpdate()\n\
    Setup()\n\
  EndEvent\n\
EndState\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_function_named_oninit() {
    let source = "ScriptName Example\n\n\
State Busy\n\
  Function OnInit()\n\
    Setup()\n\
  EndFunction\n\
EndState\n";

    assert!(check(source).is_empty());
}

#[test]
fn matches_oninit_case_insensitively() {
    let source = "ScriptName Example\n\n\
State Idle\n\
  Event oninit()\n\
  EndEvent\n\
EndState\n";

    let diagnostics = check(source);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn does_not_run_without_an_ast() {
    let diagnostics = super::check(
        DEAD_ONINIT,
        None,
        None,
        &crate::config::Config::default(),
        &mut NoExternalSignatures,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn honors_a_disable_on_the_diagnostic_line() {
    let source = "ScriptName Example\n\n\
State Busy\n\
  Event OnInit() ; @disable auto-state-oninit-misunderstanding\n\
    Setup()\n\
  EndEvent\n\
EndState\n";

    let diagnostics = crate::lint(source, &crate::config::Config::default());

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn honors_a_file_disable() {
    let source = "; @disable-file auto-state-oninit-misunderstanding\n\
ScriptName Example\n\n\
State Busy\n\
  Event OnInit()\n\
    Setup()\n\
  EndEvent\n\
EndState\n";

    let diagnostics = crate::lint(source, &crate::config::Config::default());

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn honors_the_config_off_switch() {
    let mut config = crate::config::Config::default();
    config.rules.auto_state_oninit_misunderstanding = false;

    let diagnostics = crate::lint(DEAD_ONINIT, &config);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn flags_only_non_auto_when_mixed_with_auto() {
    let source = "ScriptName Example\n\n\
Auto State Ready\n\
  Event OnInit()\n\
    Setup()\n\
  EndEvent\n\
EndState\n\n\
State Busy\n\
  Event OnInit()\n\
    Setup()\n\
  EndEvent\n\
EndState\n";

    let diagnostics = check(source);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 10);
    assert!(diagnostics[0].message.contains("Busy"));
}
