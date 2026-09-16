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
#[path = "state_count_tests.rs"]
mod tests;
