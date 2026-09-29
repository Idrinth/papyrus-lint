use super::*;
use crate::external_signatures::ExternalSignatures;

struct Events {
    answer: Option<bool>,
}

impl ExternalSignatures for Events {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<crate::ParamInfo>> {
        None
    }

    fn has_event(&mut self, type_name: &str, event_name: &str) -> Option<bool> {
        assert_eq!(type_name, "ObjectReference");
        assert!(event_name.eq_ignore_ascii_case("OnActivate"));
        self.answer
    }
}

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

fn check_with(source: &str, answer: Option<bool>) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut Events { answer },
    )
}

const EMPTY_TOP_LEVEL: &str = "ScriptName Example Extends ObjectReference\n\n\
Event OnActivate(ObjectReference akActionRef)\n\
EndEvent\n";

#[test]
fn flags_completely_empty_top_level_event() {
    let diagnostics = check(EMPTY_TOP_LEVEL);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[info]"));
    assert!(diagnostics[0].message.contains("'OnActivate'"));
    assert!(diagnostics[0].message.contains("empty body"));
}

#[test]
fn flags_when_ancestor_is_known_to_declare_the_event() {
    let diagnostics = check_with(EMPTY_TOP_LEVEL, Some(true));

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn does_not_flag_non_empty_event() {
    let source = "ScriptName Example Extends ObjectReference\n\n\
Event OnActivate(ObjectReference akActionRef)\n\
    Debug.Trace(\"activated\")\n\
EndEvent\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_empty_event_inside_named_state() {
    let source = "ScriptName Example Extends ObjectReference\n\n\
State Done\n\
    Event OnActivate(ObjectReference akActionRef)\n\
    EndEvent\n\
EndState\n";

    assert!(check(source).is_empty());
    assert!(check_with(source, Some(true)).is_empty());
}

#[test]
fn does_not_flag_empty_function() {
    let source = "ScriptName Example Extends ObjectReference\n\n\
Function Helper()\n\
EndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_event_definition_without_extends() {
    let source = "ScriptName Example\n\nEvent OnActivate(ObjectReference akActionRef)\nEndEvent\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_when_ancestor_known_not_to_declare_the_event() {
    assert!(check_with(EMPTY_TOP_LEVEL, Some(false)).is_empty());
}

#[test]
fn does_not_flag_native_event() {
    let source = "ScriptName Example Extends ObjectReference\n\n\
Event OnActivate(ObjectReference akActionRef) Native\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_events_on_engine_handled_scripts() {
    let source = "ScriptName ObjectReference Extends Form\n\n\
Function GetFormID() Native\n\n\
Event OnActivate(ObjectReference akActionRef)\n\
EndEvent\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_native_scripts() {
    let source = "ScriptName ScriptObject Native\n\nEvent OnInit()\nEndEvent\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_remote_event_handlers() {
    let source = "ScriptName Listener Extends Quest\n\n\
Event Actor.OnDeath(Actor akSender, Actor akKiller)\n\
EndEvent\n";
    let ast = papyrus_parser::parse_with_mode(
        source,
        papyrus_parser::parser::GameEdition::Fallout4,
    )
    .expect("remote event should parse");
    let diagnostics = super::check(
        source,
        Some(&ast),
        None,
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example Extends ObjectReference\n\nEvent OnActivate(\nEndEvent\n").is_empty());
}

#[test]
fn disable_directives_suppress_diagnostics() {
    let line_disabled = crate::lint(
        "ScriptName Example Extends ObjectReference\n\n\
Event OnActivate(ObjectReference akActionRef) ; @disable empty-event-handler\n\
EndEvent\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file empty-event-handler\n\
ScriptName Example Extends ObjectReference\n\n\
Event OnActivate(ObjectReference akActionRef)\n\
EndEvent\n",
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
    config.rules.empty_event_handler = false;

    let diagnostics = crate::lint(EMPTY_TOP_LEVEL, &config);

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}
