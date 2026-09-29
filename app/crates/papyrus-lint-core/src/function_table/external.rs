//! [`papyrus_lints::ExternalSignatures`] for [`super::FunctionTable`].

use papyrus_lints::ParamInfo;

use super::{CacheProbe, FunctionTable};

impl FunctionTable {
    /// Whether `type_name` is a Papyrus primitive, the compiler typedefs
    /// `CustomEventName` and `ScriptEventName`, an array of one of those
    /// (`T[]`, never a script named `T[]`), a bundled vanilla/SKSE script,
    /// or a script this table can locate. Read-only: never fills the parse
    /// cache. Nested `Script:Struct` types are not answered here; see
    /// [`papyrus_lints::ExternalSignatures::type_exists`], which loads the
    /// declaring script when the name is not itself a script.
    pub fn type_exists(&self, type_name: &str) -> bool {
        let base = array_element_name(type_name);
        is_builtin_scalar(base) || self.script_exists(base)
    }

    /// [`Self::type_exists`], plus a nested struct (`Script:Struct` or
    /// `Namespace:Script:Struct`, including an array of one) declared on a
    /// script this table can load. A namespaced script that itself exists
    /// is already accepted by [`Self::type_exists`] and is not treated as a
    /// struct. `Miss` means the declaring script still has to be loaded.
    pub(in crate::function_table) fn type_exists_cached(
        &self,
        type_name: &str,
    ) -> CacheProbe<bool> {
        if self.type_exists(type_name) {
            return CacheProbe::Hit(true);
        }
        let Some((owner, struct_name)) = struct_reference(type_name) else {
            return CacheProbe::Hit(false);
        };
        if !self.script_exists(owner) {
            return CacheProbe::Hit(false);
        }
        match self.get_cached(&owner.to_ascii_lowercase()) {
            None => CacheProbe::Miss,
            Some(None) => CacheProbe::Hit(false),
            Some(Some(script)) => {
                CacheProbe::Hit(script.structs.contains(&struct_name.to_ascii_lowercase()))
            }
        }
    }

    /// Whether `type_name` names a struct declared on another script.
    /// Loads that script when it exists and is not cached yet. A name that
    /// is itself a script (`User:Foo`) is not a struct, even when splitting
    /// on the last `:` would name one.
    pub(super) fn declared_struct_exists(&mut self, type_name: &str) -> bool {
        let base = array_element_name(type_name);
        if self.script_exists(base) {
            return false;
        }
        let Some((owner, struct_name)) = struct_reference(base) else {
            return false;
        };
        if !self.script_exists(owner) {
            return false;
        }
        let owner_key = owner.to_ascii_lowercase();
        self.ensure_loaded(&owner_key);
        self.scripts
            .get(&owner_key)
            .and_then(Option::as_ref)
            .is_some_and(|script| script.structs.contains(&struct_name.to_ascii_lowercase()))
    }

    /// Whether `type_name`'s own script declares `struct_name`. Does not
    /// walk `Extends`. Loads the script when it exists and is not cached.
    pub(in crate::function_table) fn declares_struct(
        &mut self,
        type_name: &str,
        struct_name: &str,
    ) -> bool {
        let key = type_name.to_ascii_lowercase();
        self.ensure_loaded(&key);
        self.scripts
            .get(&key)
            .and_then(Option::as_ref)
            .is_some_and(|script| script.structs.contains(&struct_name.to_ascii_lowercase()))
    }

    /// Whether `type_name` or an ancestor it `Extends` declares
    /// `struct_name`. Loads scripts along the chain. A circular `Extends`
    /// stops the walk.
    pub(in crate::function_table) fn declares_struct_in_ancestry(
        &mut self,
        type_name: &str,
        struct_name: &str,
    ) -> bool {
        let struct_key = struct_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());
        while let Some(name) = current {
            if visited.contains(&name) {
                break;
            }
            self.ensure_loaded(&name);
            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                break;
            };
            if script.structs.contains(&struct_key) {
                return true;
            }
            current = script
                .extends
                .as_ref()
                .map(|parent| parent.to_ascii_lowercase());
            visited.push(name);
        }
        false
    }

    /// [`Self::declares_struct`] answered only from a script already cached.
    pub(in crate::function_table) fn declares_struct_cached(
        &self,
        type_name: &str,
        struct_name: &str,
    ) -> CacheProbe<bool> {
        match self.get_cached(&type_name.to_ascii_lowercase()) {
            None => CacheProbe::Miss,
            Some(None) => CacheProbe::Hit(false),
            Some(Some(script)) => {
                CacheProbe::Hit(script.structs.contains(&struct_name.to_ascii_lowercase()))
            }
        }
    }
}

/// `T[]` is an array of `T`. Papyrus has no `T[][]`.
pub(in crate::function_table) fn array_element_name(type_name: &str) -> &str {
    type_name.strip_suffix("[]").unwrap_or(type_name)
}

/// Types the compiler accepts with no `.psc` file: the scalars
/// (`Int`, `Float`, `Bool`, `String`, `Var`) and the event-name typedefs
/// `CustomEventName` / `ScriptEventName` used on `ScriptObject`.
fn is_builtin_scalar(type_name: &str) -> bool {
    matches!(
        type_name.to_ascii_lowercase().as_str(),
        "int" | "float" | "bool" | "string" | "var" | "customeventname" | "scripteventname"
    )
}

/// Last `:` segment is the struct; everything before it is the declaring
/// script (`Holder:Payload`, `Namespace:Script:Payload`).
pub(in crate::function_table) fn struct_reference(type_name: &str) -> Option<(&str, &str)> {
    let base = array_element_name(type_name);
    let (owner, struct_name) = base.rsplit_once(':')?;
    if owner.is_empty() || struct_name.is_empty() {
        return None;
    }
    Some((owner, struct_name))
}

/// Lets the "Argument type check" lint (`papyrus_lints::argument_types`)
/// resolve calls to functions declared on other scripts through this
/// table.
impl papyrus_lints::ExternalSignatures for FunctionTable {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        self.lookup_function(type_name, function_name)
            .map(|signature| signature.params)
    }

    fn function_access(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<papyrus_lints::MemberAccess> {
        FunctionTable::function_access(self, type_name, function_name)
    }

    fn property_access(
        &mut self,
        type_name: &str,
        property_name: &str,
    ) -> Option<papyrus_lints::MemberAccess> {
        FunctionTable::property_access(self, type_name, property_name)
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        self.is_subtype(sub_type, super_type)
    }

    fn has_property(&mut self, type_name: &str, property_name: &str) -> bool {
        self.has_property(type_name, property_name)
    }

    fn has_field(&mut self, type_name: &str, field_name: &str) -> bool {
        self.has_field(type_name, field_name)
    }

    fn script_exists(&mut self, type_name: &str) -> bool {
        FunctionTable::script_exists(self, type_name)
    }

    fn can_resolve_script(&mut self, type_name: &str) -> bool {
        FunctionTable::script_exists(self, type_name)
    }

    fn type_exists(&mut self, type_name: &str) -> bool {
        FunctionTable::type_exists(self, type_name) || self.declared_struct_exists(type_name)
    }

    fn declares_struct(&mut self, type_name: &str, struct_name: &str) -> bool {
        FunctionTable::declares_struct(self, type_name, struct_name)
    }

    fn declares_struct_in_ancestry(&mut self, type_name: &str, struct_name: &str) -> bool {
        FunctionTable::declares_struct_in_ancestry(self, type_name, struct_name)
    }

    fn has_state(&mut self, type_name: &str, state_name: &str) -> bool {
        self.has_state(type_name, state_name)
    }

    fn ancestor_states(&mut self, type_name: &str) -> Vec<(String, bool)> {
        self.ancestor_states(type_name)
    }

    fn descendant_targets_state(&mut self, type_name: &str, state_name: &str) -> bool {
        self.descendant_targets_state(type_name, state_name)
    }

    fn has_event(&mut self, type_name: &str, event_name: &str) -> Option<bool> {
        FunctionTable::has_event(self, type_name, event_name)
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        self.lookup_function(type_name, function_name)
            .map(|signature| signature.is_global)
    }

    fn is_nodiscard_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        self.lookup_function(type_name, function_name)
            .map(|signature| signature.nodiscard)
    }

    fn deprecated_function(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<papyrus_parser::ast::Deprecation> {
        self.lookup_function(type_name, function_name)
            .and_then(|signature| signature.deprecation)
    }

    fn function_has_side_effects(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        self.lookup_function(type_name, function_name)
            .map(|signature| signature.has_side_effects)
    }

    fn function_return_type(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<papyrus_parser::ast::TypeName> {
        self.lookup_function(type_name, function_name)
            .and_then(|signature| signature.return_type)
    }

    fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        self.ancestry_fully_known(type_name)
    }

    fn property_types(&mut self, type_name: &str) -> Vec<String> {
        self.property_types(type_name)
    }

    fn registers_remote_event(&mut self, type_name: &str, event_name: &str) -> Option<bool> {
        FunctionTable::registers_remote_event(self, type_name, event_name)
    }
}

#[cfg(test)]
#[path = "external_tests.rs"]
mod tests;
