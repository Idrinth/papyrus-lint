//! Shared state-name resolution for lints that inspect the current script's states.

use std::collections::HashSet;

use papyrus_parser::ast::Script;

use crate::external_signatures::ExternalSignatures;

/// The state declarations and parent script needed to resolve a state name.
#[derive(Default)]
pub(crate) struct StateReferences {
    local_states: HashSet<String>,
    extends: Option<String>,
}

impl StateReferences {
    pub(crate) fn collect(script: &Script) -> Self {
        Self {
            local_states: script
                .states
                .iter()
                .map(|state| state.name.to_ascii_lowercase())
                .collect(),
            extends: script.extends.clone(),
        }
    }

    /// Whether `name` is neither the empty state, a local state, nor a
    /// state declared anywhere in the script's resolved `Extends` ancestry.
    pub(crate) fn is_missing<E: ExternalSignatures + ?Sized>(
        &self,
        name: &str,
        external: &mut E,
    ) -> bool {
        if name.is_empty() || self.local_states.contains(&name.to_ascii_lowercase()) {
            return false;
        }

        match self.extends.as_deref() {
            None => true,
            Some(parent) => !external.has_state(parent, name),
        }
    }
}

#[cfg(test)]
#[path = "state_reference_tests.rs"]
mod tests;
