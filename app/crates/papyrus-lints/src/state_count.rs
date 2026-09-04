//! Flags a script whose full `Extends` inheritance chain declares too many
//! named `State` blocks, or more than one of them marked `Auto`.
//!
//! Per the CreationKit wiki's [State
//! Reference](https://ck.uesp.net/wiki/State_Reference): "The game has a
//! limit of 128 states, including the empty state, meaning you can add 127
//! states to your script before the game and CK will refuse to load your
//! script and script properties," logging "Class [SCRIPT_NAME_HERE]
//! overflowed the named state count field(s) while linking. The class is
//! marked as invalid" — so [`check_too_many_states`]/[`check_too_many_states_with`]
//! flag a script whose own `State`s, combined with every `State` declared
//! anywhere in its ancestry, exceed [`MAX_NAMED_STATES`].
//!
//! The same reference also notes: "Only one state may be auto in a
//! script. A child script's auto state takes precedence over a parent's,
//! but if the child has no auto state, the parent's will be used." The
//! engine tolerates a parent and a child each declaring their own
//! (different) `Auto` state — the child's simply wins — but relying on
//! that precedence is fragile: which one actually takes effect silently
//! depends on which script the instance is, and removing the child's
//! `Auto` state (intentionally or not) silently switches the object's
//! startup state back to the parent's. [`check_multiple_auto_states`]/
//! [`check_multiple_auto_states_with`] therefore reports that cross-script
//! combination as a warning. More than one `Auto` state declared directly in
//! the same script is an error, because a script itself may only declare one.
//!
//! Both checks treat a same-named `State` declared more than once across a
//! script and its ancestry (e.g. a child overriding a parent's state) as a
//! single state, matching how [`crate::goto_state`] resolves a `GoToState`
//! target through the same ancestry.
//!
//! Resolving anything beyond the script's own declared states needs an
//! [`ExternalSignatures`] implementation (see [`check_too_many_states_with`]/
//! [`check_multiple_auto_states_with`]); [`check_too_many_states`]/
//! [`check_multiple_auto_states`] only ever see the script's own states, so
//! they can't flag a violation that only arises through inheritance.

use std::collections::HashMap;

use papyrus_parser::ast::Script;

use crate::argument_types::{ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id for the named-state-count check, for
/// `@disable` line comments.
pub const TOO_MANY_STATES_RULE: &str = "too-many-named-states";

/// This lint's [`Diagnostic::rule`] id for the multiple-`Auto`-states
/// check, for `@disable` line comments.
pub const MULTIPLE_AUTO_STATES_RULE: &str = "multiple-auto-states";

/// The engine's named-state limit per the CreationKit wiki's State
/// Reference: 128 states including the empty state, i.e. 127 named ones;
/// see the module docs.
const MAX_NAMED_STATES: usize = 127;

/// Checks `source`'s own declared `State`s for exceeding
/// [`MAX_NAMED_STATES`]. A script that `Extends` another only has its own
/// states counted here; see [`check_too_many_states_with`] to also resolve
/// its ancestry.
pub fn check_too_many_states(source: &str) -> Vec<Diagnostic> {
    check_too_many_states_with(source, &mut NoExternalSignatures)
}

/// Like [`check_too_many_states`], but also resolves every `State`
/// declared anywhere in `source`'s `Extends` ancestry through `external`
/// (see the module docs) before comparing against [`MAX_NAMED_STATES`].
pub fn check_too_many_states_with<E: ExternalSignatures>(
    source: &str,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some((script, states)) = combined_states(source, external) else {
        return Vec::new();
    };

    if states.len() > MAX_NAMED_STATES {
        vec![too_many_states(&script, states.len())]
    } else {
        Vec::new()
    }
}

/// Checks `source`'s own declared `State`s for more than one marked
/// `Auto`. A script that `Extends` another only has its own states
/// checked here; see [`check_multiple_auto_states_with`] to also resolve
/// its ancestry.
pub fn check_multiple_auto_states(source: &str) -> Vec<Diagnostic> {
    check_multiple_auto_states_with(source, &mut NoExternalSignatures)
}

/// Like [`check_multiple_auto_states`], but also resolves every `State`
/// declared anywhere in `source`'s `Extends` ancestry through `external`
/// (see the module docs) before counting how many are marked `Auto`.
pub fn check_multiple_auto_states_with<E: ExternalSignatures>(
    source: &str,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some((script, states)) = combined_states(source, external) else {
        return Vec::new();
    };

    let local_auto_count = script.states.iter().filter(|state| state.is_auto).count();
    let inherited_auto_count = states.values().filter(|&&is_auto| is_auto).count();
    if local_auto_count > 1 {
        vec![multiple_auto_states(&script, local_auto_count, true)]
    } else if inherited_auto_count > 1 {
        vec![multiple_auto_states(&script, inherited_auto_count, false)]
    } else {
        Vec::new()
    }
}

/// Parses `source` and, if it parses cleanly, returns it alongside the
/// combined set of named states (lowercased name -> whether any
/// declaration of it is `Auto`) drawn from the script's own `State`s and,
/// when it `Extends` another script, everything `external` resolves in
/// that ancestry.
fn combined_states<E: ExternalSignatures>(
    source: &str,
    external: &mut E,
) -> Option<(Script, HashMap<String, bool>)> {
    let script = papyrus_parser::parse(source).ok()?;

    let mut states: HashMap<String, bool> = HashMap::new();
    for state in &script.states {
        let is_auto = states
            .entry(state.name.to_ascii_lowercase())
            .or_insert(false);
        *is_auto |= state.is_auto;
    }
    if let Some(parent) = &script.extends {
        for (name, is_auto) in external.ancestor_states(parent) {
            let entry = states.entry(name.to_ascii_lowercase()).or_insert(false);
            *entry |= is_auto;
        }
    }

    Some((script, states))
}

/// Anchors a whole-script diagnostic at the last locally-declared state's
/// line, or line 1 if the script declares no states of its own (i.e. the
/// violation comes entirely from its `Extends` ancestry).
fn anchor(script: &Script) -> usize {
    match script.states.last() {
        Some(state) => state.line,
        None => 1,
    }
}

fn too_many_states(script: &Script, count: usize) -> Diagnostic {
    Diagnostic {
        line: anchor(script),
        column: 1,
        message: format!(
            "[error] Script '{}' declares {count} named states across its inheritance chain, exceeding the engine's limit of {MAX_NAMED_STATES}",
            script.name,
        ),
        rule: TOO_MANY_STATES_RULE,
    }
}

fn multiple_auto_states(script: &Script, count: usize, is_local_error: bool) -> Diagnostic {
    let message = if is_local_error {
        format!(
            "[error] Script '{}' declares {count} states marked Auto, but a script may only declare one Auto state",
            script.name,
        )
    } else {
        format!(
            "[warning] Script '{}' has {count} states marked Auto across its inheritance chain; its startup state depends on inheritance precedence",
            script.name,
        )
    };
    Diagnostic {
        line: anchor(script),
        column: 1,
        message,
        rule: MULTIPLE_AUTO_STATES_RULE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let source =
            "ScriptName Example\n\nAuto State Idle\nEndState\n\nAuto State Active\nEndState\n";
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
        let source =
            "ScriptName Example\n\nAuto State Idle\nEndState\n\nAuto State Idle\nEndState\n";
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
        ) -> Option<Vec<crate::argument_types::ParamInfo>> {
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
}
