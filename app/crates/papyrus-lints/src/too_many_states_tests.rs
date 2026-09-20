use super::*;
use crate::external_signatures::{ExternalSignatures, NoExternalSignatures, ParamInfo};

fn source_with_states(count: usize, last_state_comment: Option<&str>) -> String {
    let mut source = "ScriptName Example\n\n".to_string();
    for index in 0..count {
        source.push_str(&format!("State State{index}"));
        if index + 1 == count {
            if let Some(comment) = last_state_comment {
                source.push_str(comment);
            }
        }
        source.push_str("\nEndState\n");
    }
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
fn dispatches_the_named_state_count_check() {
    let source = source_with_states(128, None);

    let diagnostics = check(&source, &mut NoExternalSignatures);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 257);
    assert!(diagnostics[0].message.contains("128 named states"));
}

#[test]
fn does_not_run_without_an_ast() {
    let source = source_with_states(128, None);

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
    let source = source_with_states(128, Some(" ; @disable too-many-named-states"));

    let diagnostics = crate::lint(&source, &crate::config::Config::default());

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn honors_a_file_disable() {
    let mut source = "; @disable-file too-many-named-states\n".to_string();
    source.push_str(&source_with_states(128, None));

    let diagnostics = crate::lint(&source, &crate::config::Config::default());

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

struct InheritedStates;

impl ExternalSignatures for InheritedStates {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn ancestor_states(&mut self, type_name: &str) -> Vec<(String, bool)> {
        assert_eq!(type_name, "ParentScript");
        (0..128)
            .map(|index| (format!("InheritedState{index}"), false))
            .collect()
    }
}

#[test]
fn forwards_external_signatures_to_the_state_count_check() {
    let source = "ScriptName Example Extends ParentScript\n";

    let diagnostics = check(source, &mut InheritedStates);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 1);
    assert!(diagnostics[0].message.contains("128 named states"));
}
