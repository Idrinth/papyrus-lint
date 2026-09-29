use super::*;
use crate::config::Config;
use crate::external_signatures::NoExternalSignatures;
use crate::Game;

fn parse_fo4(source: &str) -> papyrus_parser::ast::Script {
    papyrus_parser::parse_with_mode(source, papyrus_parser::parser::GameEdition::Fallout4)
        .expect("remote event should parse in Fallout 4 dialect")
}

fn check_with(source: &str, config: &Config) -> Vec<Diagnostic> {
    let ast = parse_fo4(source);
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        Some(&ast),
        tokens.as_deref(),
        config,
        &mut NoExternalSignatures,
    )
}

fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &Config::default())
}

fn fo4() -> Config {
    Config {
        game: Game::Fallout4,
        ..Default::default()
    }
}

fn lint_fo4(source: &str, config: &Config) -> Vec<Diagnostic> {
    let ast = parse_fo4(source);
    papyrus_parser::prime_cache(source, ast);
    crate::lint(source, config)
}


#[test]
fn flags_remote_handler_without_registration() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("ObjectReference.OnCellAttach"));
    assert!(diagnostics[0].message.contains("RegisterForRemoteEvent"));
}

#[test]
fn does_not_flag_when_same_script_registers_matching_event() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnInit()\n    RegisterForRemoteEvent(akTarget, \"OnCellAttach\")\nEndEvent\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_when_named_argument_matches() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Start(ObjectReference akTarget)\n    RegisterForRemoteEvent(akSelf = akTarget, eventName = \"OnCellAttach\")\nEndFunction\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_when_registration_event_is_not_a_literal() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Start(ObjectReference akTarget, ScriptEventName eventName)\n    RegisterForRemoteEvent(akTarget, eventName)\nEndFunction\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_when_registered_event_name_does_not_match() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnInit()\n    RegisterForRemoteEvent(akTarget, \"OnDeath\")\nEndEvent\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("OnCellAttach"));
}

#[test]
fn flags_qualified_register_call_on_receiver() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnInit()\n    self.RegisterForRemoteEvent(akTarget, \"OnCellAttach\")\nEndEvent\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_ordinary_event() {
    let diagnostics = check("ScriptName Example\n\nEvent OnInit()\nEndEvent\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_treat_unregister_as_registration() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnInit()\n    UnregisterForRemoteEvent(akTarget, \"OnCellAttach\")\nEndEvent\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn matches_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnInit()\n    registerforremoteevent(akTarget, \"oncellattach\")\nEndEvent\n\nEvent objectreference.oncellattach(ObjectReference akSender)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_remote_handler_inside_named_state() {
    let diagnostics = check(
        "ScriptName Example\n\nState Busy\n    Event ObjectReference.OnCellAttach(ObjectReference akSender)\n    EndEvent\nEndState\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn respects_disable_comment() {
    let source = "ScriptName Example\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender) ; @disable unregistered-remote-event\nEndEvent\n";
    assert_eq!(check(source).len(), 1);
    let filtered = lint_fo4(source, &fo4());
    assert!(filtered.iter().all(|d| d.rule != RULE));
}

#[test]
fn lint_runs_for_fallout4() {
    let diagnostics = lint_fo4(
        "ScriptName Example\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
        &fo4(),
    );
    assert!(diagnostics.iter().any(|d| d.rule == RULE));
}

#[test]
fn lint_runs_for_starfield() {
    let config = Config {
        game: Game::Starfield,
        ..Default::default()
    };
    let diagnostics = lint_fo4(
        "ScriptName Example\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
        &config,
    );
    assert!(diagnostics.iter().any(|d| d.rule == RULE));
}

#[test]
fn lint_skips_when_game_is_skyrim() {
    let diagnostics = crate::lint(
        "ScriptName Example\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
        &Config::default(),
    );
    assert!(diagnostics.iter().all(|d| d.rule != RULE));
}
