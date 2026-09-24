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
        assert_eq!(type_name, "ParentScript");
        assert!(event_name.eq_ignore_ascii_case("OnSomething"));
        self.answer
    }
}

fn check(source: &str, answer: Option<bool>) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::Config::default(),
        &mut Events { answer },
    )
}

const CHILD_EVENT: &str =
    "ScriptName Child Extends ParentScript\n\nEvent OnSomething()\nEndEvent\n";

#[test]
fn flags_an_event_missing_from_a_known_ancestry() {
    let diagnostics = check(CHILD_EVENT, Some(false));

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("'OnSomething'"));
}

#[test]
fn accepts_an_event_declared_by_an_ancestor() {
    assert!(check(CHILD_EVENT, Some(true)).is_empty());
}

#[test]
fn skips_an_event_when_the_ancestry_cannot_be_resolved() {
    assert!(check(CHILD_EVENT, None).is_empty());
}

#[test]
fn skips_an_event_on_a_script_with_no_resolvable_ancestor() {
    assert!(check("ScriptName Root\n\nEvent Anything()\nEndEvent\n", None).is_empty());
}

#[test]
fn ignores_functions() {
    let source = "ScriptName Child Extends ParentScript\n\nFunction OnSomething()\nEndFunction\n";
    assert!(check(source, Some(false)).is_empty());
}

#[test]
fn checks_events_inside_states() {
    let source = "ScriptName Child Extends ParentScript\n\nState Busy\n    Event OnSomething()\n    EndEvent\nEndState\n";
    let diagnostics = check(source, Some(false));
    assert_eq!(diagnostics[0].line, 4);
}

fn lint_rule(source: &str, enabled: bool) -> Vec<Diagnostic> {
    let mut config = crate::Config::default();
    config.rules.undefined_event = enabled;
    crate::lint_with_external_arguments(source, &config, &mut Events { answer: Some(false) })
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == RULE)
        .collect()
}

#[test]
fn respects_disable_comments() {
    let line_disabled = "ScriptName Child Extends ParentScript\n\nEvent OnSomething() ; @disable undefined-event\nEndEvent\n";
    assert!(lint_rule(line_disabled, true).is_empty());

    let file_disabled = "ScriptName Child Extends ParentScript\n; @disable-file undefined-event\nEvent OnSomething()\nEndEvent\n";
    assert!(lint_rule(file_disabled, true).is_empty());
}

#[test]
fn respects_the_config_switch() {
    assert!(lint_rule(CHILD_EVENT, false).is_empty());
}
