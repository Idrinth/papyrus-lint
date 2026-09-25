//! `ExternalSignatures` forwarding for `SharedFunctionTable`.

use papyrus_lints::ParamInfo;

use super::{FunctionTable, SharedFunctionTable};

impl papyrus_lints::ExternalSignatures for SharedFunctionTable<'_> {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        self.probe_or_load(
            |table| {
                table
                    .lookup_function_cached(type_name, function_name)
                    .map(|signature| signature.map(|signature| signature.params))
            },
            |table| papyrus_lints::ExternalSignatures::lookup(table, type_name, function_name),
        )
    }

    fn function_access(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<papyrus_lints::MemberAccess> {
        self.probe_or_load(
            |table| {
                table.member_access_cached(type_name, function_name, |script, key| {
                    script.functions.get(key).map(|member| member.access_level)
                })
            },
            |table| {
                papyrus_lints::ExternalSignatures::function_access(table, type_name, function_name)
            },
        )
    }

    fn property_access(
        &mut self,
        type_name: &str,
        property_name: &str,
    ) -> Option<papyrus_lints::MemberAccess> {
        self.probe_or_load(
            |table| {
                table.member_access_cached(type_name, property_name, |script, key| {
                    script.properties.get(key).map(|member| member.access_level)
                })
            },
            |table| {
                papyrus_lints::ExternalSignatures::property_access(table, type_name, property_name)
            },
        )
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        self.probe_or_load(
            |table| table.is_subtype_cached(sub_type, super_type),
            |table| papyrus_lints::ExternalSignatures::is_subtype(table, sub_type, super_type),
        )
    }

    fn has_property(&mut self, type_name: &str, property_name: &str) -> bool {
        self.probe_or_load(
            |table| table.has_property_cached(type_name, property_name),
            |table| {
                papyrus_lints::ExternalSignatures::has_property(table, type_name, property_name)
            },
        )
    }

    fn has_field(&mut self, type_name: &str, field_name: &str) -> bool {
        self.probe_or_load(
            |table| table.has_field_cached(type_name, field_name),
            |table| papyrus_lints::ExternalSignatures::has_field(table, type_name, field_name),
        )
    }

    fn script_exists(&mut self, type_name: &str) -> bool {
        FunctionTable::script_exists(&self.read(), type_name)
    }

    fn can_resolve_script(&mut self, type_name: &str) -> bool {
        FunctionTable::script_exists(&self.read(), type_name)
    }

    fn type_exists(&mut self, type_name: &str) -> bool {
        FunctionTable::type_exists(&self.read(), type_name)
    }

    fn has_state(&mut self, type_name: &str, state_name: &str) -> bool {
        self.probe_or_load(
            |table| table.has_state_cached(type_name, state_name),
            |table| papyrus_lints::ExternalSignatures::has_state(table, type_name, state_name),
        )
    }

    fn ancestor_states(&mut self, type_name: &str) -> Vec<(String, bool)> {
        self.probe_or_load(
            |table| table.ancestor_states_cached(type_name),
            |table| papyrus_lints::ExternalSignatures::ancestor_states(table, type_name),
        )
    }

    fn descendant_targets_state(&mut self, type_name: &str, state_name: &str) -> bool {
        self.probe_or_load(
            |table| table.descendant_targets_state_cached(type_name, state_name),
            |table| {
                papyrus_lints::ExternalSignatures::descendant_targets_state(
                    table, type_name, state_name,
                )
            },
        )
    }

    fn has_event(&mut self, type_name: &str, event_name: &str) -> Option<bool> {
        self.probe_or_load(
            |table| table.has_event_cached(type_name, event_name),
            |table| papyrus_lints::ExternalSignatures::has_event(table, type_name, event_name),
        )
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        self.probe_or_load(
            |table| {
                table
                    .lookup_function_cached(type_name, function_name)
                    .map(|signature| signature.map(|signature| signature.is_global))
            },
            |table| {
                papyrus_lints::ExternalSignatures::is_global_function(
                    table,
                    type_name,
                    function_name,
                )
            },
        )
    }

    fn is_nodiscard_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        self.probe_or_load(
            |table| {
                table
                    .lookup_function_cached(type_name, function_name)
                    .map(|signature| signature.map(|signature| signature.nodiscard))
            },
            |table| {
                papyrus_lints::ExternalSignatures::is_nodiscard_function(
                    table,
                    type_name,
                    function_name,
                )
            },
        )
    }

    fn deprecated_function(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<papyrus_parser::ast::Deprecation> {
        self.probe_or_load(
            |table| {
                table
                    .lookup_function_cached(type_name, function_name)
                    .map(|signature| signature.and_then(|signature| signature.deprecation.clone()))
            },
            |table| {
                papyrus_lints::ExternalSignatures::deprecated_function(
                    table,
                    type_name,
                    function_name,
                )
            },
        )
    }

    fn function_has_side_effects(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        self.probe_or_load(
            |table| {
                table
                    .lookup_function_cached(type_name, function_name)
                    .map(|signature| signature.map(|signature| signature.has_side_effects))
            },
            |table| {
                papyrus_lints::ExternalSignatures::function_has_side_effects(
                    table,
                    type_name,
                    function_name,
                )
            },
        )
    }

    fn function_return_type(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<papyrus_parser::ast::TypeName> {
        self.probe_or_load(
            |table| {
                table
                    .lookup_function_cached(type_name, function_name)
                    .map(|signature| signature.and_then(|signature| signature.return_type.clone()))
            },
            |table| {
                papyrus_lints::ExternalSignatures::function_return_type(
                    table,
                    type_name,
                    function_name,
                )
            },
        )
    }

    fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        self.probe_or_load(
            |table| table.ancestry_fully_known_cached(type_name),
            |table| papyrus_lints::ExternalSignatures::ancestry_fully_known(table, type_name),
        )
    }

    fn property_types(&mut self, type_name: &str) -> Vec<String> {
        self.probe_or_load(
            |table| table.property_types_cached(type_name),
            |table| papyrus_lints::ExternalSignatures::property_types(table, type_name),
        )
    }
}
