//! A thread-safe [`papyrus_lints::ExternalSignatures`] adapter over a
//! [`super::FunctionTable`] shared by multiple lint workers.

use std::sync::Mutex;

use papyrus_lints::ParamInfo;

use super::FunctionTable;

/// A thread-safe [`ExternalSignatures`](papyrus_lints::ExternalSignatures)
/// adapter over a [`FunctionTable`] shared by multiple lint workers at once
/// (see [`crate::parallel`]): each trait method locks the underlying table
/// only for the duration of that one lookup, rather than for a whole lint
/// pass, so concurrently linted scripts only ever contend with each other
/// at the comparatively rare moment one actually needs another script's
/// signature -- typically a single lock acquisition per referenced type,
/// since `FunctionTable` caches everything it resolves -- not for the
/// CPU-bound parsing/rule work surrounding it.
///
/// Every method is forwarded through the [`ExternalSignatures`] trait
/// itself (via fully qualified syntax), rather than reimplementing this
/// type's logic (e.g. `type_exists`'s primitive-type/native-type fallback,
/// which isn't a plain delegate to an inherent method), so this adapter
/// can never drift out of sync with `FunctionTable`'s own trait impl.
pub struct SharedFunctionTable<'a>(pub &'a Mutex<FunctionTable>);

impl papyrus_lints::ExternalSignatures for SharedFunctionTable<'_> {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::lookup(&mut *table, type_name, function_name)
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::is_subtype(&mut *table, sub_type, super_type)
    }

    fn has_property(&mut self, type_name: &str, property_name: &str) -> bool {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::has_property(&mut *table, type_name, property_name)
    }

    fn has_field(&mut self, type_name: &str, field_name: &str) -> bool {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::has_field(&mut *table, type_name, field_name)
    }

    fn script_exists(&mut self, type_name: &str) -> bool {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::script_exists(&mut *table, type_name)
    }

    fn can_resolve_script(&mut self, type_name: &str) -> bool {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::can_resolve_script(&mut *table, type_name)
    }

    fn type_exists(&mut self, type_name: &str) -> bool {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::type_exists(&mut *table, type_name)
    }

    fn has_state(&mut self, type_name: &str, state_name: &str) -> bool {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::has_state(&mut *table, type_name, state_name)
    }

    fn ancestor_states(&mut self, type_name: &str) -> Vec<(String, bool)> {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::ancestor_states(&mut *table, type_name)
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::is_global_function(&mut *table, type_name, function_name)
    }

    fn is_nodiscard_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::is_nodiscard_function(
            &mut *table,
            type_name,
            function_name,
        )
    }

    fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::ancestry_fully_known(&mut *table, type_name)
    }

    fn property_types(&mut self, type_name: &str) -> Vec<String> {
        let mut table = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        papyrus_lints::ExternalSignatures::property_types(&mut *table, type_name)
    }
}

#[cfg(test)]
#[path = "shared_tests.rs"]
mod tests;
