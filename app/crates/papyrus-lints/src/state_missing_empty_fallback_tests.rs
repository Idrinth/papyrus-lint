use super::*;
use crate::external_signatures::{ExternalSignatures, NoExternalSignatures, ParamInfo};

fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

fn check_with(source: &str, external: &mut impl ExternalSignatures) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        external,
    )
}

const STATE_ONLY: &str = "ScriptName Example\n\n\
State Active\n\
  Function DoWork()\n\
  EndFunction\n\
EndState\n";

struct ParentFns {
    known: bool,
    names: &'static [&'static str],
}

impl ExternalSignatures for ParentFns {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn has_empty_state_function(&mut self, _type_name: &str, function_name: &str) -> Option<bool> {
        if !self.known {
            return None;
        }
        Some(
            self.names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(function_name)),
        )
    }
}

#[test]
fn flags_function_declared_only_in_a_named_state() {
    let diagnostics = check(STATE_ONLY);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("DoWork"));
    assert!(diagnostics[0].message.contains("Active"));
}

#[test]
fn flags_event_declared_only_in_a_named_state() {
    let source = "ScriptName Example\n\n\
State Active\n\
  Event OnActivate(ObjectReference akActionRef)\n\
  EndEvent\n\
EndState\n";

    let diagnostics = check(source);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Event"));
    assert!(diagnostics[0].message.contains("OnActivate"));
}

#[test]
fn does_not_flag_when_the_empty_state_declares_the_same_name() {
    let source = "ScriptName Example\n\n\
Function DoWork()\n\
EndFunction\n\n\
State Active\n\
  Function DoWork()\n\
  EndFunction\n\
EndState\n";

    assert!(check(source).is_empty());
}

#[test]
fn matches_the_empty_state_name_case_insensitively() {
    let source = "ScriptName Example\n\n\
Function dowork()\n\
EndFunction\n\n\
State Active\n\
  Function DoWork()\n\
  EndFunction\n\
EndState\n";

    assert!(check(source).is_empty());
}

#[test]
fn flags_each_state_that_lacks_an_empty_state_declaration() {
    let source = "ScriptName Example\n\n\
State Active\n\
  Function DoWork()\n\
  EndFunction\n\
EndState\n\n\
State Idle\n\
  Function DoWork()\n\
  EndFunction\n\
EndState\n";

    let diagnostics = check(source);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[1].line, 9);
}

#[test]
fn does_not_flag_a_remote_event() {
    let source = "ScriptName Example\n\n\
State Active\n\
  Event Actor.OnDeath(Actor akKiller)\n\
  EndEvent\n\
EndState\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_a_private_helper_only_called_inside_its_state() {
    let source = "ScriptName Example\n\n\
State Active\n\
  Function Run()\n\
    Helper()\n\
  EndFunction\n\n\
  Function Helper() ; @private\n\
  EndFunction\n\
EndState\n";

    let diagnostics = check(source);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Run"));
    assert!(diagnostics.iter().all(|d| !d.message.contains("Helper")));
}

#[test]
fn does_not_flag_an_uncalled_private_annotation() {
    let source = "ScriptName Example\n\n\
State Active\n\
  ; @private\n\
  Function Helper()\n\
  EndFunction\n\
EndState\n";

    assert!(check(source).is_empty());
}

#[test]
fn flags_a_private_helper_called_from_the_empty_state() {
    let source = "ScriptName Example\n\n\
Function Run()\n\
  Helper()\n\
EndFunction\n\n\
State Active\n\
  Function Helper() ; @private\n\
  EndFunction\n\
EndState\n";

    let diagnostics = check(source);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Helper"));
}

#[test]
fn flags_a_private_helper_called_from_another_state() {
    let source = "ScriptName Example\n\n\
State Idle\n\
  Function Run()\n\
    Helper()\n\
  EndFunction\n\
EndState\n\n\
State Active\n\
  Function Helper() ; @private\n\
  EndFunction\n\
EndState\n";

    let diagnostics = check(source);
    assert!(diagnostics.iter().any(|d| d.message.contains("Helper")));
}

#[test]
fn parent_empty_state_declaration_suppresses_the_diagnostic() {
    let source = "ScriptName Example Extends QuestScript\n\n\
State Active\n\
  Function DoWork()\n\
  EndFunction\n\
EndState\n";
    let mut external = ParentFns {
        known: true,
        names: &["DoWork"],
    };

    assert!(check_with(source, &mut external).is_empty());
}

#[test]
fn resolved_parent_without_the_function_still_flags() {
    let source = "ScriptName Example Extends QuestScript\n\n\
State Active\n\
  Function DoWork()\n\
  EndFunction\n\
EndState\n";
    let mut external = ParentFns {
        known: true,
        names: &["Other"],
    };

    assert_eq!(check_with(source, &mut external).len(), 1);
}

#[test]
fn unresolved_parent_is_not_guessed() {
    let source = "ScriptName Example Extends QuestScript\n\n\
State Active\n\
  Function DoWork()\n\
  EndFunction\n\
EndState\n";
    let mut external = ParentFns {
        known: false,
        names: &[],
    };

    assert!(check_with(source, &mut external).is_empty());
}

#[test]
fn does_not_run_without_an_ast() {
    let diagnostics = super::check(
        STATE_ONLY,
        None,
        None,
        &crate::config::Config::default(),
        &mut NoExternalSignatures,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn disable_directives_suppress_diagnostics() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\n\
State Active\n\
  Function DoWork() ; @disable state-missing-empty-fallback\n\
  EndFunction\n\
EndState\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file state-missing-empty-fallback\n\
ScriptName Example\n\n\
State Active\n\
  Function DoWork()\n\
  EndFunction\n\
EndState\n",
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
    let mut config = crate::config::Config::default();
    config.rules.state_missing_empty_fallback = false;

    let diagnostics = crate::lint(STATE_ONLY, &config);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}
