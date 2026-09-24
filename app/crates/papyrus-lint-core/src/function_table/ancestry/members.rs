//! Function, property, field, and general member lookups.

use std::collections::HashSet;

use super::{cached_script, parent_cache_key, CacheProbe, FunctionTable};
use crate::script_functions::{FunctionSignature, Member, ScriptFunctions};
use papyrus_lints::MemberAccess;

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
    pub(in crate::function_table) fn member_access_cached(
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
}
