use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::argument_types::NoExternalSignatures,
    )
}

#[test]
fn compiled_known_events_are_loaded_from_yaml() {
    assert!(!KNOWN_EVENTS.is_empty());
    assert!(KNOWN_EVENTS
        .iter()
        .any(|rule| rule.event == "OnInit" && rule.form == "ScriptObject"));
}

#[test]
fn does_not_flag_a_matching_no_argument_event() {
    let diagnostics = check("ScriptName Example\n\nEvent OnInit()\nEndEvent\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_matching_event_with_arguments() {
    let diagnostics =
        check("ScriptName Example\n\nEvent OnActivate(ObjectReference akActionRef)\nEndEvent\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn matches_case_insensitively() {
    let diagnostics = check("ScriptName Example\n\nevent oninit()\nendevent\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_missing_argument() {
    let diagnostics = check("ScriptName Example\n\nEvent OnActivate()\nEndEvent\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("OnActivate"));
    assert!(diagnostics[0].message.contains("ObjectReference"));
}

#[test]
fn flags_a_mismatched_argument_type() {
    let diagnostics =
        check("ScriptName Example\n\nEvent OnActivate(Actor akActionRef)\nEndEvent\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("OnActivate"));
}

#[test]
fn flags_an_extra_argument() {
    let diagnostics = check("ScriptName Example\n\nEvent OnInit(Int aiUnexpected)\nEndEvent\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("OnInit"));
}

#[test]
fn flags_an_event_declared_inside_a_state() {
    let diagnostics =
        check("ScriptName Example\n\nState Busy\n    Event OnActivate()\n    EndEvent\nEndState\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_an_unrelated_event_name() {
    let diagnostics = check("ScriptName Example\n\nEvent MyCustomEvent(Int aiValue)\nEndEvent\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_function_with_the_same_name_as_an_event() {
    let diagnostics = check("ScriptName Example\n\nFunction OnActivate()\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn matches_every_known_event_with_its_own_correct_signature() {
    for rule in KNOWN_EVENTS {
        let params = rule
            .args
            .iter()
            .map(|arg| format!("{} {}", arg.type_name, arg.name))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            "ScriptName Example\n\nEvent {}({params})\nEndEvent\n",
            rule.event
        );
        let diagnostics = check(&source);
        assert!(
            diagnostics.is_empty(),
            "{} was unexpectedly flagged",
            rule.event
        );
    }
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nEvent OnInit(\n");
    assert!(diagnostics.is_empty());
}
