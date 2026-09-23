//! `Extends`-chain lookups against a [`super::FunctionTable`]: functions,
//! properties, states, members, and subtype/ancestry queries.

use std::collections::HashSet;

use super::{CacheProbe, FunctionTable};
use crate::script_functions::{FunctionSignature, Member, ScriptFunctions};
use papyrus_lints::MemberAccess;

fn cached_script<'a>(
    table: &'a FunctionTable,
    name: &str,
) -> CacheProbe<Option<&'a ScriptFunctions>> {
    match table.get_cached(name) {
        None => CacheProbe::Miss,
        Some(slot) => CacheProbe::Hit(slot.as_ref()),
    }
}

/// ASCII-lowercased `Extends` parent, matching [`FunctionTable::ensure_loaded`]'s
/// cache keys. Walks that used the declared casing as the next `current`
/// name would miss a previously loaded lowercase slot (or insert a second
/// one) whenever `Extends Actor` and a later `actor` lookup mixed.
fn parent_cache_key(script: &ScriptFunctions) -> Option<String> {
    script
        .extends
        .as_ref()
        .map(|parent| parent.to_ascii_lowercase())
}

impl FunctionTable {
    pub fn function_access(&mut self, type_name: &str, member_name: &str) -> Option<MemberAccess> {
        self.member_access(type_name, member_name, |script, key| {
            script.functions.get(key).map(|member| member.access_level)
        })
    }

    pub fn property_access(&mut self, type_name: &str, member_name: &str) -> Option<MemberAccess> {
        self.member_access(type_name, member_name, |script, key| {
            script.properties.get(key).map(|member| member.access_level)
        })
    }

    fn member_access(
        &mut self,
        type_name: &str,
        member_name: &str,
        find: impl Fn(&ScriptFunctions, &str) -> Option<papyrus_parser::ast::AccessLevel>,
    ) -> Option<MemberAccess> {
        let key = member_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());
        while let Some(name) = current {
            if visited.contains(&name) {
                break;
            }
            self.ensure_loaded(&name);
            let script = self.scripts.get(&name)?.as_ref()?;
            if let Some(access_level) = find(script, &key) {
                return Some(MemberAccess {
                    declaring_type: name,
                    access_level,
                });
            }
            current = parent_cache_key(script);
            visited.push(name);
        }
        None
    }

    /// [`Self::member_access`] answered only from scripts already cached.
    /// [`CacheProbe::Miss`] means some ancestor still needs
    /// [`Self::ensure_loaded`].
    pub(super) fn member_access_cached(
        &self,
        type_name: &str,
        member_name: &str,
        find: impl Fn(&ScriptFunctions, &str) -> Option<papyrus_parser::ast::AccessLevel>,
    ) -> CacheProbe<Option<MemberAccess>> {
        let key = member_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());
        while let Some(name) = current {
            if visited.contains(&name) {
                return CacheProbe::Hit(None);
            }
            let script = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(None) => return CacheProbe::Hit(None),
                CacheProbe::Hit(Some(script)) => script,
            };
            if let Some(access_level) = find(script, &key) {
                return CacheProbe::Hit(Some(MemberAccess {
                    declaring_type: name,
                    access_level,
                }));
            }
            current = parent_cache_key(script);
            visited.push(name);
        }
        CacheProbe::Hit(None)
    }

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

            current = parent_cache_key(script);
            visited.push(name);
        }

        None
    }

    /// Whether `sub_type`'s script is, or extends (directly or
    /// transitively), `super_type`. Both names are matched
    /// case-insensitively. When a type along the way isn't a script in the
    /// configured script roots, falls back to the bundled vanilla/SKSE
    /// AST cache by `ScriptName`. Returns `false` once a script in the
    /// chain cannot be resolved before reaching `super_type`.
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

            current = self
                .scripts
                .get(&name)
                .and_then(Option::as_ref)
                .and_then(parent_cache_key);
            visited.push(name);
        }

        false
    }

    /// Whether `type_name`'s full `Extends` ancestry resolves all the way to
    /// a definite root: a script found (and parsed) with no `Extends` line
    /// at all. Matched case-insensitively, mirroring
    /// [`Self::is_subtype`]'s own walk (including the bundled vanilla/SKSE
    /// name fallback), but tracking whether
    /// the walk actually reached a confirmed root rather than just whether
    /// it reached a particular type. A circular `Extends` chain, or a type
    /// along the way this table has no data for at all (not a project
    /// script, not in the bundled cache) means the ancestry is *not*
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
                Some(script) => match parent_cache_key(script) {
                    Some(parent) => Some(parent),
                    None => return true, // an explicit script with no Extends is a definite root
                },
                None => return false,
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

            current = parent_cache_key(script);
            visited.push(name);
        }

        false
    }

    /// Whether `type_name`'s script, or an ancestor it `Extends` (directly
    /// or transitively), declares a script-level variable (a plain field,
    /// not a `Property`) named `field_name`. Both names are matched
    /// case-insensitively. Returns `false` if `type_name`'s script (or any
    /// ancestor along the way) can't be found or parsed before a match is
    /// found. Used by the "Local variable shadowing" lint
    /// (`papyrus_lints::local_variable_shadowing`) to check a local
    /// variable against a parent script's fields, mirroring
    /// [`Self::has_property`] above.
    pub fn has_field(&mut self, type_name: &str, field_name: &str) -> bool {
        let field_key = field_name.to_ascii_lowercase();
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
            if script.variables.contains(&field_key) {
                return true;
            }

            current = parent_cache_key(script);
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

            current = parent_cache_key(script);
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

            current = parent_cache_key(script);
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

            current = parent_cache_key(script);
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

    pub(super) fn lookup_function_cached(
        &self,
        type_name: &str,
        function_name: &str,
    ) -> CacheProbe<Option<FunctionSignature>> {
        let function_key = function_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break;
            }
            let script = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(None) => return CacheProbe::Hit(None),
                CacheProbe::Hit(Some(script)) => script,
            };
            if let Some(signature) = script.functions.get(&function_key) {
                return CacheProbe::Hit(Some(signature.clone()));
            }
            current = parent_cache_key(script);
            visited.push(name);
        }

        CacheProbe::Hit(None)
    }

    pub(super) fn is_subtype_cached(&self, sub_type: &str, super_type: &str) -> CacheProbe<bool> {
        let super_lower = super_type.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(sub_type.to_ascii_lowercase());

        while let Some(name) = current {
            if name == super_lower {
                return CacheProbe::Hit(true);
            }
            if visited.contains(&name) {
                break;
            }
            current = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(Some(script)) => script
                    .extends
                    .as_ref()
                    .map(|parent| parent.to_ascii_lowercase()),
                CacheProbe::Hit(None) => None,
            };
            visited.push(name);
        }

        CacheProbe::Hit(false)
    }

    pub(super) fn ancestry_fully_known_cached(&self, type_name: &str) -> CacheProbe<bool> {
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                return CacheProbe::Hit(false);
            }
            current = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(Some(script)) => match &script.extends {
                    Some(parent) => Some(parent.to_ascii_lowercase()),
                    None => return CacheProbe::Hit(true),
                },
                CacheProbe::Hit(None) => return CacheProbe::Hit(false),
            };
            visited.push(name);
        }

        CacheProbe::Hit(false)
    }

    pub(super) fn has_property_cached(
        &self,
        type_name: &str,
        property_name: &str,
    ) -> CacheProbe<bool> {
        self.has_member_cached(type_name, |script| {
            script
                .properties
                .contains_key(&property_name.to_ascii_lowercase())
        })
    }

    pub(super) fn has_field_cached(&self, type_name: &str, field_name: &str) -> CacheProbe<bool> {
        self.has_member_cached(type_name, |script| {
            script.variables.contains(&field_name.to_ascii_lowercase())
        })
    }

    pub(super) fn has_state_cached(&self, type_name: &str, state_name: &str) -> CacheProbe<bool> {
        self.has_member_cached(type_name, |script| {
            script.states.contains_key(&state_name.to_ascii_lowercase())
        })
    }

    fn has_member_cached(
        &self,
        type_name: &str,
        found: impl Fn(&ScriptFunctions) -> bool,
    ) -> CacheProbe<bool> {
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break;
            }
            let script = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(None) => return CacheProbe::Hit(false),
                CacheProbe::Hit(Some(script)) => script,
            };
            if found(script) {
                return CacheProbe::Hit(true);
            }
            current = parent_cache_key(script);
            visited.push(name);
        }

        CacheProbe::Hit(false)
    }

    pub(super) fn ancestor_states_cached(
        &self,
        type_name: &str,
    ) -> CacheProbe<Vec<(String, bool)>> {
        let mut result = Vec::new();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break;
            }
            let script = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(None) => break,
                CacheProbe::Hit(Some(script)) => script,
            };
            result.extend(
                script
                    .states
                    .iter()
                    .map(|(state, &is_auto)| (state.clone(), is_auto)),
            );
            current = script
                .extends
                .as_ref()
                .map(|parent| parent.to_ascii_lowercase());
            visited.push(name);
        }

        CacheProbe::Hit(result)
    }

    pub(super) fn list_members_cached(&self, type_name: &str) -> CacheProbe<Vec<Member>> {
        let mut seen = HashSet::new();
        let mut members = Vec::new();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break;
            }
            let script = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(None) => break,
                CacheProbe::Hit(Some(script)) => script,
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
            current = parent_cache_key(script);
            visited.push(name);
        }

        CacheProbe::Hit(members)
    }

    pub(super) fn property_types_cached(&self, type_name: &str) -> CacheProbe<Vec<String>> {
        let name_lower = type_name.to_ascii_lowercase();
        match cached_script(self, &name_lower) {
            CacheProbe::Miss => CacheProbe::Miss,
            CacheProbe::Hit(None) => CacheProbe::Hit(Vec::new()),
            CacheProbe::Hit(Some(script)) => CacheProbe::Hit(
                script
                    .properties
                    .values()
                    .map(|property| property.type_name.name.clone())
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
#[path = "ancestry_tests.rs"]
mod tests;
