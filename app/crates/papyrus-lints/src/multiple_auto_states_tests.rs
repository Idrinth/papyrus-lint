use super::*;
use crate::external_signatures::{ExternalSignatures, NoExternalSignatures, ParamInfo};

fn source_with_auto_states(last_state_comment: Option<&str>) -> String {
    let mut source = "ScriptName Example\n\nAuto State Idle\nEndState\n\nAuto State Active".to_string();
    if let Some(comment) = last_state_comment {
        source.push_str(comment);
    }
    source.push_str("\nEndState\n");
    source
}

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

#[test]
fn dispatches_the_multiple_auto_states_check() {
    let source = source_with_auto_states(None);

    let diagnostics = check(&source, &mut NoExternalSignatures);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 6);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("2 states marked Auto"));
}

#[test]
fn does_not_run_without_an_ast() {
    let source = source_with_auto_states(None);

    let diagnostics = super::check(
        &source,
        None,
        None,
        &crate::config::Config::default(),
        &mut NoExternalSignatures,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn honors_a_disable_on_the_diagnostic_line() {
    let source = source_with_auto_states(Some(" ; @disable multiple-auto-states"));

    let diagnostics = crate::lint(&source, &crate::config::Config::default());

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn honors_a_file_disable() {
    let mut source = "; @disable-file multiple-auto-states\n".to_string();
    source.push_str(&source_with_auto_states(None));

    let diagnostics = crate::lint(&source, &crate::config::Config::default());

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn honors_the_config_off_switch() {
    let source = source_with_auto_states(None);
    let mut config = crate::config::Config::default();
    config.rules.multiple_auto_states = false;

    let diagnostics = crate::lint(&source, &config);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

struct InheritedAutoState;

impl ExternalSignatures for InheritedAutoState {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn ancestor_states(&mut self, type_name: &str) -> Vec<(String, bool)> {
        assert_eq!(type_name, "ParentScript");
        vec![("ParentIdle".to_string(), true)]
    }
}

#[test]
fn forwards_external_signatures_to_the_state_count_check() {
    let source = "ScriptName Example Extends ParentScript\n\nAuto State LocalIdle\nEndState\n";

    let diagnostics = check(source, &mut InheritedAutoState);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("2 states marked Auto"));
}
