//! [`papyrus_lints::ExternalSignatures`] for [`super::FunctionTable`].

use papyrus_lints::ParamInfo;

use super::FunctionTable;

impl FunctionTable {
    /// Whether `type_name` is a Papyrus primitive, a bundled vanilla/SKSE
    /// script, or a script this table can locate. Read-only: never fills
    /// the parse cache.
    pub fn type_exists(&self, type_name: &str) -> bool {
        let name_lower = type_name.to_ascii_lowercase();
        matches!(
            name_lower.as_str(),
            "int" | "float" | "bool" | "string" | "var"
        ) || self.script_exists(type_name)
    }
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
        FunctionTable::type_exists(self, type_name)
    }

    fn has_state(&mut self, type_name: &str, state_name: &str) -> bool {
        self.has_state(type_name, state_name)
    }

    fn ancestor_states(&mut self, type_name: &str) -> Vec<(String, bool)> {
        self.ancestor_states(type_name)
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        self.lookup_function(type_name, function_name)
            .map(|signature| signature.is_global)
    }

    fn is_nodiscard_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        self.lookup_function(type_name, function_name)
            .map(|signature| signature.nodiscard)
    }

    fn is_deprecated_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        self.lookup_function(type_name, function_name)
            .map(|signature| signature.deprecated)
    }

    fn function_has_side_effects(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        self.lookup_function(type_name, function_name)
            .map(|signature| signature.has_side_effects)
    }

    fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        self.ancestry_fully_known(type_name)
    }

    fn property_types(&mut self, type_name: &str) -> Vec<String> {
        self.property_types(type_name)
    }
}

#[cfg(test)]
#[path = "external_tests.rs"]
mod tests;
