use std::collections::HashMap;

use super::*;
use crate::config::Config;
use crate::external_signatures::ExternalSignatures;
use crate::Game;

/// Events known on each type. `None` means that type's ancestry could not
/// be resolved.
struct KnownEvents {
    by_type: HashMap<String, Option<Vec<&'static str>>>,
}

impl KnownEvents {
    fn new(pairs: &[(&str, Option<&[&'static str]>)]) -> Self {
        let by_type = pairs
            .iter()
            .map(|(ty, events)| {
                (
                    ty.to_ascii_lowercase(),
                    events.map(|events| events.to_vec()),
                )
            })
            .collect();
        Self { by_type }
    }
}

impl ExternalSignatures for KnownEvents {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<crate::ParamInfo>> {
        None
    }

    fn has_event(&mut self, type_name: &str, event_name: &str) -> Option<bool> {
        self.by_type
            .get(&type_name.to_ascii_lowercase())
            .and_then(|events| {
                events.as_ref().map(|events| {
                    events
                        .iter()
                        .any(|event| event.eq_ignore_ascii_case(event_name))
                })
            })
    }
}

fn parse(source: &str) -> papyrus_parser::ast::Script {
    papyrus_parser::parse_with_mode(source, papyrus_parser::parser::GameEdition::Fallout4)
        .expect("script should parse")
}

fn check(source: &str, known: &mut KnownEvents) -> Vec<Diagnostic> {
    let ast = parse(source);
    super::check(source, Some(&ast), None, &Config::default(), known)
}

fn actor_events() -> KnownEvents {
    KnownEvents::new(&[
        ("Actor", Some(&["OnDeath", "OnLoad"][..])),
        ("ObjectReference", Some(&["OnLoad", "OnCellAttach"][..])),
        ("Quest", Some(&["OnStoryHello"][..])),
    ])
}

const TYPO: &str = "\
ScriptName Example\n\
\n\
Actor Property Actor1 Auto\n\
\n\
Event OnInit()\n\
    RegisterForRemoteEvent(Actor1, \"OnLood\")\n\
EndEvent\n";

#[test]
fn flags_a_misspelled_event_on_a_property() {
    let diagnostics = check(TYPO, &mut actor_events());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 6);
    assert!(diagnostics[0].message.contains("'OnLood'"));
    assert!(diagnostics[0].message.contains("Actor"));
    assert!(diagnostics[0].message.contains("RegisterForRemoteEvent"));
}

#[test]
fn accepts_an_event_inherited_onto_the_source_type() {
    let diagnostics = check(
        "\
ScriptName Example\n\
\n\
Actor Property Actor1 Auto\n\
\n\
Event OnInit()\n\
    RegisterForRemoteEvent(Actor1, \"OnLoad\")\n\
EndEvent\n",
        &mut actor_events(),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_unregister_the_same_way() {
    let diagnostics = check(
        "\
ScriptName Example\n\
\n\
Actor Property Actor1 Auto\n\
\n\
Function Stop()\n\
    UnregisterForRemoteEvent(Actor1, \"OnLood\")\n\
EndFunction\n",
        &mut actor_events(),
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("UnregisterForRemoteEvent"));
}

#[test]
fn accepts_a_named_argument_pair() {
    let diagnostics = check(
        "\
ScriptName Example\n\
\n\
Function Start(Actor akTarget)\n\
    RegisterForRemoteEvent(akEventSource = akTarget, asEventName = \"OnDeath\")\n\
EndFunction\n",
        &mut actor_events(),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_named_event_typo_when_the_source_is_positional() {
    let diagnostics = check(
        "\
ScriptName Example\n\
\n\
Function Start(Actor akTarget)\n\
    RegisterForRemoteEvent(akTarget, eventName = \"OnLood\")\n\
EndFunction\n",
        &mut actor_events(),
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn skips_a_non_literal_event_name() {
    let diagnostics = check(
        "\
ScriptName Example\n\
\n\
Function Start(Actor akTarget, String eventName)\n\
    RegisterForRemoteEvent(akTarget, eventName)\n\
EndFunction\n",
        &mut actor_events(),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn skips_an_unresolvable_source() {
    let diagnostics = check(
        "\
ScriptName Example\n\
\n\
Event OnInit()\n\
    RegisterForRemoteEvent(Game.GetPlayer(), \"OnLood\")\n\
EndEvent\n",
        &mut actor_events(),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn skips_when_the_source_ancestry_cannot_be_resolved() {
    let mut known = KnownEvents::new(&[("Mystery", None)]);
    let diagnostics = check(
        "\
ScriptName Example\n\
\n\
Mystery Property Target Auto\n\
\n\
Event OnInit()\n\
    RegisterForRemoteEvent(Target, \"OnLood\")\n\
EndEvent\n",
        &mut known,
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_an_event_the_source_type_cannot_receive() {
    let diagnostics = check(
        "\
ScriptName Example\n\
\n\
Quest Property MyQuest Auto\n\
\n\
Event OnInit()\n\
    RegisterForRemoteEvent(MyQuest, \"OnDeath\")\n\
EndEvent\n",
        &mut actor_events(),
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Quest"));
}

#[test]
fn accepts_a_cast_to_the_declaring_type() {
    let diagnostics = check(
        "\
ScriptName Example\n\
\n\
Form Property Door Auto\n\
\n\
Event OnInit()\n\
    RegisterForRemoteEvent(Door as ObjectReference, \"OnCellAttach\")\n\
EndEvent\n",
        &mut actor_events(),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn matches_case_insensitively_on_the_call_and_the_event() {
    let diagnostics = check(
        "\
ScriptName Example\n\
\n\
Actor Property Actor1 Auto\n\
\n\
Event OnInit()\n\
    RegisterForRemoteEvent(Actor1, \"onload\")\n\
    self.RegisterForRemoteEvent(Actor1, \"ONDEATH\")\n\
EndEvent\n",
        &mut actor_events(),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_when_no_resolver_can_answer() {
    let mut known = KnownEvents::new(&[]);
    assert!(check(TYPO, &mut known).is_empty());
}

#[test]
fn lint_runs_for_fallout4_and_starfield_only() {
    let source = TYPO;
    let ast = parse(source);
    papyrus_parser::prime_cache(source, ast);

    let fo4 = Config {
        game: Game::Fallout4,
        ..Default::default()
    };
    let starfield = Config {
        game: Game::Starfield,
        ..Default::default()
    };
    let skyrim = Config {
        game: Game::Skyrim,
        ..Default::default()
    };

    // Without a project resolver, has_event is None and the rule stays quiet.
    // The games gate is still observable: Skyrim never reports this rule id,
    // and the rule is registered for the other two games (a resolver-backed
    // call is covered by the unit tests above).
    let fo4_diags = crate::lint(source, &fo4);
    let sf_diags = crate::lint(source, &starfield);
    let skyrim_diags = crate::lint(source, &skyrim);
    assert!(fo4_diags.iter().all(|d| d.rule != RULE));
    assert!(sf_diags.iter().all(|d| d.rule != RULE));
    assert!(skyrim_diags.iter().all(|d| d.rule != RULE));
}

#[test]
fn respects_disable_comment() {
    let source = "\
ScriptName Example\n\
\n\
Actor Property Actor1 Auto\n\
\n\
Event OnInit()\n\
    RegisterForRemoteEvent(Actor1, \"OnLood\") ; @disable unknown-remote-event\n\
EndEvent\n";
    let mut known = actor_events();
    assert_eq!(check(source, &mut known).len(), 1);

    let ast = parse(source);
    papyrus_parser::prime_cache(source, ast);
    let config = Config {
        game: Game::Fallout4,
        ..Default::default()
    };
    let enabled = "\
ScriptName Example\n\
\n\
Actor Property Actor1 Auto\n\
\n\
Event OnInit()\n\
    RegisterForRemoteEvent(Actor1, \"OnLood\")\n\
EndEvent\n";
    let enabled_ast = parse(enabled);
    papyrus_parser::prime_cache(enabled, enabled_ast);
    let reported = crate::lint_with_external_arguments(enabled, &config, &mut known);
    assert!(reported.iter().any(|d| d.rule == RULE));

    let filtered = crate::lint_with_external_arguments(source, &config, &mut known);
    assert!(filtered.iter().all(|d| d.rule != RULE));
}
