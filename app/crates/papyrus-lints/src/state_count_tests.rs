use super::*;

fn check_too_many_states(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_too_many_states(ast.as_ref())
}

fn check_too_many_states_with<E: ExternalSignatures>(
    source: &str,
    external: &mut E,
) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_too_many_states_with(ast.as_ref(), external)
}

fn check_multiple_auto_states(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_multiple_auto_states(ast.as_ref())
}

fn check_multiple_auto_states_with<E: ExternalSignatures>(
    source: &str,
    external: &mut E,
) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_multiple_auto_states_with(ast.as_ref(), external)
}

fn state_block(name: &str, auto: bool) -> String {
    let prefix = if auto { "Auto State" } else { "State" };
    format!("{prefix} {name}\nEndState\n")
}

fn script_with_states(count: usize) -> String {
    script_extending_with_states(None, count)
}

fn script_extending_with_states(extends: Option<&str>, count: usize) -> String {
    let header = match extends {
        Some(parent) => format!("ScriptName Example Extends {parent}\n\n"),
        None => "ScriptName Example\n\n".to_string(),
    };
    let mut source = header;
    for index in 0..count {
        source.push_str(&state_block(&format!("State{index}"), false));
    }
    source
}

#[test]
fn does_not_flag_a_script_at_the_named_state_limit() {
    let source = script_with_states(127);
    assert!(check_too_many_states(&source).is_empty());
}

#[test]
fn does_not_flag_a_script_without_named_states() {
    let source = "ScriptName Example\n";

    assert!(check_too_many_states(source).is_empty());
    assert!(check_multiple_auto_states(source).is_empty());
}

#[test]
fn flags_a_script_that_exceeds_the_named_state_limit() {
    let source = script_with_states(128);
    let diagnostics = check_too_many_states(&source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, TOO_MANY_STATES_RULE);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("128 named states"));
    assert!(diagnostics[0].message.contains("limit of 127"));
}

#[test]
fn does_not_flag_a_single_auto_state() {
    let source = "ScriptName Example\n\nAuto State Idle\nEndState\n\nState Active\nEndState\n";
    assert!(check_multiple_auto_states(source).is_empty());
}

#[test]
fn flags_more_than_one_local_auto_state() {
    let source = "ScriptName Example\n\nAuto State Idle\nEndState\n\nAuto State Active\nEndState\n";
    let diagnostics = check_multiple_auto_states(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, MULTIPLE_AUTO_STATES_RULE);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("2 states marked Auto"));
    assert!(diagnostics[0]
        .message
        .contains("a script may only declare one Auto state"));
}

#[test]
fn counts_duplicate_local_auto_declarations_as_an_error() {
    let source = "ScriptName Example\n\nAuto State Idle\nEndState\n\nAuto State Idle\nEndState\n";
    let diagnostics = check_multiple_auto_states(source);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("2 states marked Auto"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let source = "ScriptName Example\n\nState (((\nEndState\n";
    assert!(check_too_many_states(source).is_empty());
    assert!(check_multiple_auto_states(source).is_empty());
}

#[test]
fn bare_check_only_counts_the_scripts_own_states() {
    // `check_too_many_states` has no resolver to consult, so it can
    // only ever see states declared directly on this script; see
    // `counts_inherited_states_toward_the_limit` below for the
    // `_with` variant that also resolves ancestry.
    let source = script_with_states(10);
    assert!(check_too_many_states(&source).is_empty());
}

struct FakeExternalWithAncestorStates {
    states: Vec<(String, bool)>,
}

impl ExternalSignatures for FakeExternalWithAncestorStates {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn ancestor_states(&mut self, _type_name: &str) -> Vec<(String, bool)> {
        self.states.clone()
    }
}

#[test]
fn counts_inherited_states_toward_the_limit() {
    let source = script_extending_with_states(Some("ParentScript"), 0);
    let mut external = FakeExternalWithAncestorStates {
        states: (0..127)
            .map(|index| (format!("ParentState{index}"), false))
            .collect(),
    };

    assert!(check_too_many_states_with(&source, &mut external).is_empty());

    external.states.push(("OneMore".to_string(), false));
    let diagnostics = check_too_many_states_with(&source, &mut external);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("128 named states"));
}

#[test]
fn does_not_double_count_a_state_overridden_from_a_parent() {
    // 127 locally-declared states, one of which (`State0`) is also
    // declared on the parent: if the same-named state were counted
    // twice instead of once, this would total 128 and get flagged.
    let source = script_extending_with_states(Some("ParentScript"), 127);
    let mut external = FakeExternalWithAncestorStates {
        states: vec![("State0".to_string(), false)],
    };

    assert!(check_too_many_states_with(&source, &mut external).is_empty());
}

#[test]
fn matches_overridden_state_names_case_insensitively() {
    let source = script_extending_with_states(Some("ParentScript"), 127);
    let mut external = FakeExternalWithAncestorStates {
        states: vec![("sTaTe0".to_string(), false)],
    };

    assert!(check_too_many_states_with(&source, &mut external).is_empty());
}

#[test]
fn deduplicates_repeated_inherited_state_names() {
    let source = script_extending_with_states(Some("ParentScript"), 0);
    let mut external = FakeExternalWithAncestorStates {
        states: (0..128)
            .map(|index| (format!("ParentState{index}"), false))
            .chain(std::iter::once(("parentstate0".to_string(), true)))
            .collect(),
    };

    let diagnostics = check_too_many_states_with(&source, &mut external);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("128 named states"));
}

#[test]
fn flags_an_inherited_auto_state_combined_with_a_local_one() {
    let source = "ScriptName Example Extends ParentScript\n\nAuto State Local\nEndState\n";
    let mut external = FakeExternalWithAncestorStates {
        states: vec![("ParentAuto".to_string(), true)],
    };

    let diagnostics = check_multiple_auto_states_with(source, &mut external);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("2 states marked Auto"));
    assert!(diagnostics[0]
        .message
        .contains("across its inheritance chain"));
}

#[test]
fn treats_an_overridden_states_auto_flag_as_shared() {
    // The same named state is Auto on the parent but not redeclared as
    // Auto locally; since they represent one conceptual state, this is
    // still just one Auto state overall, not a conflict.
    let source = "ScriptName Example Extends ParentScript\n\nState Shared\nEndState\n";
    let mut external = FakeExternalWithAncestorStates {
        states: vec![("Shared".to_string(), true)],
    };

    assert!(check_multiple_auto_states_with(source, &mut external).is_empty());
}

#[test]
fn combines_duplicate_inherited_auto_flags_with_logical_or() {
    let source = "ScriptName Example Extends ParentScript\n";
    let mut external = FakeExternalWithAncestorStates {
        states: vec![
            ("Idle".to_string(), false),
            ("IDLE".to_string(), true),
            ("Active".to_string(), true),
        ],
    };

    let diagnostics = check_multiple_auto_states_with(source, &mut external);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[0].column, 1);
    assert_eq!(diagnostics[0].rule, MULTIPLE_AUTO_STATES_RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("2 states marked Auto"));
}

#[test]
fn local_auto_state_error_takes_precedence_over_inherited_warning() {
    let source =
            "ScriptName Example Extends ParentScript\n\nAuto State Idle\nEndState\n\nAuto State Active\nEndState\n";
    let mut external = FakeExternalWithAncestorStates {
        states: vec![("ParentAuto".to_string(), true)],
    };

    let diagnostics = check_multiple_auto_states_with(source, &mut external);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("2 states marked Auto"));
    assert!(!diagnostics[0].message.contains("inheritance precedence"));
}

#[test]
fn anchors_at_the_last_local_state_when_present() {
    let source = script_with_states(128);
    let diagnostics = check_too_many_states(&source);

    assert_eq!(diagnostics[0].line, 128 * 2 + 1);
}

#[test]
fn anchors_at_line_one_when_the_overflow_is_entirely_inherited() {
    let source = "ScriptName Example Extends ParentScript\n";
    let mut external = FakeExternalWithAncestorStates {
        states: (0..128)
            .map(|index| (format!("ParentState{index}"), false))
            .collect(),
    };

    let diagnostics = check_too_many_states_with(source, &mut external);
    assert_eq!(diagnostics[0].line, 1);
}
