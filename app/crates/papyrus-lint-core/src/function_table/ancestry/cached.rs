//! Read-only ancestry lookups used by the shared function table.

use std::collections::HashSet;

use super::events::event_chain_fresh;
use super::{cached_script, parent_cache_key, CacheProbe, FunctionTable};
use crate::script_functions::{FunctionSignature, Member, ScriptFunctions};

impl FunctionTable {
    pub(in crate::function_table) fn lookup_function_cached(
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

    pub(in crate::function_table) fn has_event_cached(
        &self,
        type_name: &str,
        event_name: &str,
    ) -> CacheProbe<Option<bool>> {
        if !self.event_roots_match() {
            return CacheProbe::Miss;
        }
        let type_key = type_name.to_ascii_lowercase();
        let Some(entry) = self.event_index.get(&type_key) else {
            return CacheProbe::Miss;
        };
        if !event_chain_fresh(entry) {
            return CacheProbe::Miss;
        }
        let event_key = event_name.to_ascii_lowercase();
        CacheProbe::Hit(Some(entry.names.contains(&event_key)))
    }

    pub(in crate::function_table) fn is_subtype_cached(
        &self,
        sub_type: &str,
        super_type: &str,
    ) -> CacheProbe<bool> {
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

    pub(in crate::function_table) fn ancestry_fully_known_cached(
        &self,
        type_name: &str,
    ) -> CacheProbe<bool> {
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

    pub(in crate::function_table) fn has_property_cached(
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

    pub(in crate::function_table) fn has_field_cached(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> CacheProbe<bool> {
        self.has_member_cached(type_name, |script| {
            script.variables.contains(&field_name.to_ascii_lowercase())
        })
    }

    pub(in crate::function_table) fn has_state_cached(
        &self,
        type_name: &str,
        state_name: &str,
    ) -> CacheProbe<bool> {
        self.has_member_cached(type_name, |script| {
            script.states.contains_key(&state_name.to_ascii_lowercase())
        })
    }

    pub(in crate::function_table) fn descendant_targets_state_cached(
        &self,
        type_name: &str,
        state_name: &str,
    ) -> CacheProbe<bool> {
        let Some(index) = &self.descendant_goto_targets else {
            return CacheProbe::Miss;
        };
        CacheProbe::Hit(
            index
                .get(&type_name.to_ascii_lowercase())
                .is_some_and(|targets| targets.contains(&state_name.to_ascii_lowercase())),
        )
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

    pub(in crate::function_table) fn ancestor_states_cached(
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

    pub(in crate::function_table) fn list_members_cached(
        &self,
        type_name: &str,
    ) -> CacheProbe<Vec<Member>> {
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

    pub(in crate::function_table) fn property_types_cached(
        &self,
        type_name: &str,
    ) -> CacheProbe<Vec<String>> {
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
