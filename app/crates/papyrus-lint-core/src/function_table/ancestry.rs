//! `Extends`-chain lookups against a [`super::FunctionTable`]: functions,
//! properties, states, members, and subtype/ancestry queries.

use std::collections::HashSet;

use super::FunctionTable;
use crate::script_functions::{FunctionSignature, Member};

impl FunctionTable {
    /// Looks up the signature of `function_name` as callable on an object
    /// of type `type_name`, searching `type_name` and its ancestors in
    /// `Extends` order.
    ///
    /// Returns `None` if neither `type_name` nor any ancestor declares a
    /// matching function, or if `type_name`'s script can't be found or
    /// parsed. Both names are matched case-insensitively.
    pub fn lookup_function(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<FunctionSignature> {
        let function_key = function_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            let script = self.scripts.get(&name)?.as_ref()?;
            if let Some(signature) = script.functions.get(&function_key) {
                return Some(signature.clone());
            }

            current = script.extends.clone();
            visited.push(name);
        }

        None
    }

    /// Whether `sub_type`'s script is, or extends (directly or
    /// transitively), `super_type`. Both names are matched
    /// case-insensitively. When a type along the way isn't a script in the
    /// project (e.g. a native engine type like `Actor` or `ObjectReference`,
    /// whose own `Extends` chain isn't declared anywhere in the project),
    /// falls back to [`crate::native_types::parent_of`] for it rather than
    /// giving up; returns `false` only once neither the project nor that
    /// fallback can say what a type in the chain extends before reaching
    /// `super_type`.
    pub fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        let super_lower = super_type.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(sub_type.to_ascii_lowercase());

        while let Some(name) = current {
            if name == super_lower {
                return true;
            }
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            current = match self.scripts.get(&name).and_then(Option::as_ref) {
                Some(script) => script.extends.as_ref().map(|e| e.to_ascii_lowercase()),
                None => crate::native_types::parent_of(&name).map(str::to_string),
            };
            visited.push(name);
        }

        false
    }

    /// Whether `type_name`'s full `Extends` ancestry resolves all the way to
    /// a definite root: a script found (and parsed) with no `Extends` line
    /// at all, or — once project resolution runs out — a native engine type
    /// from [`crate::native_types`] with no further parent. Matched
    /// case-insensitively, mirroring [`Self::is_subtype`]'s own walk (and
    /// falling back to [`crate::native_types::parent_of`] the same way past
    /// the point where project resolution runs out), but tracking whether
    /// the walk actually reached a confirmed root rather than just whether
    /// it reached a particular type. A circular `Extends` chain, or a type
    /// along the way this table has no data for at all (not a project
    /// script, not in the native fallback), means the ancestry is *not*
    /// fully known. Used by the "Impossible cast" lint
    /// (`papyrus_lints::impossible_cast`) to tell a value/target pair
    /// *proven* unrelated (both sides fully resolved, per
    /// [`papyrus_lints::ExternalSignatures::ancestry_fully_known`])
    /// apart from one this table simply doesn't have enough information
    /// about.
    pub fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                return false; // circular Extends chain; never confidently resolved
            }
            self.ensure_loaded(&name);

            current = match self.scripts.get(&name).and_then(Option::as_ref) {
                Some(script) => match &script.extends {
                    Some(parent) => Some(parent.to_ascii_lowercase()),
                    None => return true, // an explicit script with no Extends is a definite root
                },
                None => match crate::native_types::parent_of(&name) {
                    Some(parent) => Some(parent.to_string()),
                    // No further parent: only a genuine root if the native
                    // table actually knows this type at all.
                    None => return crate::native_types::is_known(&name),
                },
            };
            visited.push(name);
        }

        false
    }

    /// Whether `type_name`'s script, or an ancestor it `Extends` (directly
    /// or transitively), declares a property named `property_name`. Both
    /// names are matched case-insensitively. Returns `false` if
    /// `type_name`'s script (or any ancestor along the way) can't be found
    /// or parsed before a match is found.
    pub fn has_property(&mut self, type_name: &str, property_name: &str) -> bool {
        let property_key = property_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                break;
            };
            if script.properties.contains_key(&property_key) {
                return true;
            }

            current = script.extends.clone();
            visited.push(name);
        }

        false
    }

    /// Whether `type_name`'s script, or an ancestor it `Extends` (directly
    /// or transitively), declares a `State` block named `state_name`. Both
    /// names are matched case-insensitively. Returns `false` if
    /// `type_name`'s script (or any ancestor along the way) can't be found
    /// or parsed before a match is found. Used by the "GoToState state
    /// reference" lint (`papyrus_lints::goto_state`) to flag a
    /// `GoToState("Name")` call whose target state can't be resolved
    /// anywhere in the script's own ancestry.
    pub fn has_state(&mut self, type_name: &str, state_name: &str) -> bool {
        let state_key = state_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                break;
            };
            if script.states.contains_key(&state_key) {
                return true;
            }

            current = script.extends.clone();
            visited.push(name);
        }

        false
    }

    /// Every named `State` declared anywhere in `type_name`'s own
    /// `Extends` ancestry — `type_name`'s own script, then each further
    /// ancestor it extends — as `(name, is_auto)` pairs. Both a script's
    /// own casing (not lowercased) and every declaration it makes are
    /// included; a script (or an ancestor along the way) that can't be
    /// found or parsed simply ends the walk there rather than failing the
    /// whole lookup, mirroring [`Self::has_state`]. Used by the "Total
    /// named state count"/"Multiple Auto states" lint pair
    /// (`papyrus_lints::state_count`) to tally a script's full inheritance
    /// chain against the engine's per-script limits.
    pub fn ancestor_states(&mut self, type_name: &str) -> Vec<(String, bool)> {
        let mut result = Vec::new();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                break;
            };
            result.extend(
                script
                    .states
                    .iter()
                    .map(|(name, &is_auto)| (name.clone(), is_auto)),
            );

            // Lowercased, unlike the other walks in this file: those only
            // ever check for a match or stop at the first one found, so a
            // casing mismatch against `visited` costs at most a redundant
            // extra step. This walk instead accumulates every step's
            // states, where the same mismatch would double-count an
            // ancestor's states whenever its `Extends` target's declared
            // casing doesn't match `visited`'s.
            current = script.extends.as_ref().map(|e| e.to_ascii_lowercase());
            visited.push(name);
        }

        result
    }

    /// Lists every function and property available on an object of type
    /// `type_name`, including those inherited via `Extends`. A member
    /// declared on `type_name` itself (or an ancestor closer to it) shadows
    /// a same-named member further up the chain, so each name appears at
    /// most once. Returns an empty list if `type_name`'s script can't be
    /// found or parsed. Members are returned in no particular order.
    pub fn list_members(&mut self, type_name: &str) -> Vec<Member> {
        let mut seen = HashSet::new();
        let mut members = Vec::new();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                break;
            };

            for signature in script.functions.values() {
                if seen.insert(signature.name.to_ascii_lowercase()) {
                    members.push(Member::Function(signature.clone()));
                }
            }
            for signature in script.properties.values() {
                if seen.insert(signature.name.to_ascii_lowercase()) {
                    members.push(Member::Property(signature.clone()));
                }
            }

            current = script.extends.clone();
            visited.push(name);
        }

        members
    }

    /// Every property type declared directly on `type_name`'s own script,
    /// in its original case, not extended through `Extends`. Returns an
    /// empty list if `type_name`'s script can't be found or parsed. Used by
    /// the "Circular script dependency" lint
    /// (`papyrus_lints::circular_dependency`) to follow a chain of
    /// `Property` declarations across scripts looking for one that leads
    /// back to the script it started from.
    pub fn property_types(&mut self, type_name: &str) -> Vec<String> {
        let name_lower = type_name.to_ascii_lowercase();
        self.ensure_loaded(&name_lower);

        let Some(script) = self.scripts.get(&name_lower).and_then(Option::as_ref) else {
            return Vec::new();
        };
        script
            .properties
            .values()
            .map(|property| property.type_name.name.clone())
            .collect()
    }
}

#[cfg(test)]
#[path = "ancestry_tests.rs"]
mod tests;
