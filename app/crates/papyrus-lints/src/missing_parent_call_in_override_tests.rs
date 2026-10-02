use super::*;

use crate::external_signatures::{ExternalSignatures, ParamInfo};

fn check(source: &str, external: &mut impl ExternalSignatures) -> Vec<Diagnostic> {
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

/// `OnInit` and `OnHit` need a parent call. `OnLoad` is a resolved noop
/// (empty parent event). Anything else on `ModBase` resolved as absent.
struct ParentEvents;

impl ExternalSignatures for ParentEvents {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn parent_event_needs_call(&mut self, type_name: &str, event_name: &str) -> Option<bool> {
        if !type_name.eq_ignore_ascii_case("ModBase") {
            return None;
        }
        match event_name.to_ascii_lowercase().as_str() {
            "oninit" | "onhit" | "onplayerloadgame" => Some(true),
            "onload" => Some(false),
            _ => Some(false),
        }
    }
}

const CHILD_ON_INIT: &str = "\
ScriptName Child Extends ModBase

Event OnInit()
    MySetup()
EndEvent
";

#[test]
fn without_a_resolved_parent_never_flags() {
    let diagnostics = check(
        CHILD_ON_INIT,
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_an_event_override_that_skips_the_parent_call() {
    let diagnostics = check(CHILD_ON_INIT, &mut ParentEvents);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].column, 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Event 'OnInit'"));
    assert!(diagnostics[0].message.contains("Parent.OnInit()"));
    assert!(diagnostics[0].message.contains("'ModBase'"));
}

#[test]
fn flags_every_event_name_not_only_lifecycle_events() {
    let diagnostics = check(
        "ScriptName Child Extends ModBase\n\nEvent OnHit(ObjectReference akAggressor)\n    MySetup()\nEndEvent\n",
        &mut ParentEvents,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Parent.OnHit()"));
}

#[test]
fn does_not_flag_when_the_parent_event_is_a_noop() {
    let diagnostics = check(
        "ScriptName Child Extends ModBase\n\nEvent OnLoad()\n    MySetup()\nEndEvent\n",
        &mut ParentEvents,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_event_the_parent_does_not_declare() {
    let diagnostics = check(
        "ScriptName Child Extends ModBase\n\nEvent OnActivate(ObjectReference akActionRef)\nEndEvent\n",
        &mut ParentEvents,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn parent_call_anywhere_in_the_body_is_quiet() {
    let direct = check(
        "ScriptName Child Extends ModBase\n\nEvent OnInit()\n    Parent.OnInit()\n    MySetup()\nEndEvent\n",
        &mut ParentEvents,
    );
    let nested = check(
        "ScriptName Child Extends ModBase\n\nEvent OnInit()\n    If ready\n        parent.oninit()\n    EndIf\nEndEvent\n",
        &mut ParentEvents,
    );
    let other_event = check(
        "ScriptName Child Extends ModBase\n\nEvent OnInit()\n    Parent.OnLoad()\nEndEvent\n",
        &mut ParentEvents,
    );

    assert!(direct.is_empty());
    assert!(nested.is_empty());
    assert_eq!(other_event.len(), 1);
}

#[test]
fn no_parent_call_annotation_on_the_header_or_the_line_above_is_quiet() {
    let header = check(
        "ScriptName Child Extends ModBase\n\nEvent OnInit() ; @no-parent-call\n    MySetup()\nEndEvent\n",
        &mut ParentEvents,
    );
    let above = check(
        "ScriptName Child Extends ModBase\n\n; @No-Parent-Call\nEvent OnPlayerLoadGame()\nEndEvent\n",
        &mut ParentEvents,
    );
    let lookalike = check(
        "ScriptName Child Extends ModBase\n\nEvent OnInit() ; @no-parent-callable\nEndEvent\n",
        &mut ParentEvents,
    );

    assert!(header.is_empty());
    assert!(above.is_empty());
    assert_eq!(lookalike.len(), 1);
}

#[test]
fn does_not_flag_functions_state_events_or_scripts_without_extends() {
    let function = check(
        "ScriptName Child Extends ModBase\n\nFunction OnInit()\nEndFunction\n",
        &mut ParentEvents,
    );
    let state_event = check(
        "ScriptName Child Extends ModBase\n\nState Loud\n    Event OnInit()\n    EndEvent\nEndState\n",
        &mut ParentEvents,
    );
    let no_extends = check(
        "ScriptName Child\n\nEvent OnInit()\nEndEvent\n",
        &mut ParentEvents,
    );

    assert!(function.is_empty());
    assert!(state_event.is_empty());
    assert!(no_extends.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check(
        "ScriptName Child Extends ModBase\n\nEvent OnInit(\nEndEvent\n",
        &mut ParentEvents,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn disable_directives_suppress_diagnostics() {
    let line_disabled = crate::lint_with_external_arguments(
        "ScriptName Child Extends ModBase\n\n\
         Event OnInit() ; @disable missing-parent-call-in-override\n\
         EndEvent\n",
        &crate::config::Config::default(),
        &mut ParentEvents,
    );
    let file_disabled = crate::lint_with_external_arguments(
        "; @disable-file missing-parent-call-in-override\n\
         ScriptName Child Extends ModBase\n\n\
         Event OnInit()\n\
         EndEvent\n",
        &crate::config::Config::default(),
        &mut ParentEvents,
    );

    assert!(line_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
    assert!(file_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_off_switch_suppresses_diagnostics() {
    let mut config = crate::config::Config::default();
    config.rules.missing_parent_call_in_override = false;

    let diagnostics =
        crate::lint_with_external_arguments(CHILD_ON_INIT, &config, &mut ParentEvents);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}
