//! A thread-safe [`papyrus_lints::ExternalSignatures`] adapter over a
//! [`super::FunctionTable`] shared by multiple lint workers.

use std::sync::RwLock;

use papyrus_lints::ParamInfo;

use super::{CacheProbe, FunctionTable, Member};

/// A thread-safe [`ExternalSignatures`](papyrus_lints::ExternalSignatures)
/// adapter over a [`FunctionTable`] shared by multiple lint workers at once
/// (see [`crate::parallel`]).
///
/// Lookups that can be answered from scripts already in the table take a
/// shared read lock. The exclusive write lock is taken only when a lookup
/// still needs to locate/parse a script (`ensure_loaded`). Concurrent
/// workers therefore overlap on the CPU-bound lint work *and* on cache
/// hits, and only serialize on a cache fill.
///
/// Load misses still forward through the [`ExternalSignatures`] trait on
/// [`FunctionTable`] (via fully qualified syntax) so this adapter cannot
/// drift from that impl's policy (e.g. `type_exists`'s primitive-type /
/// native-type fallback).
pub struct SharedFunctionTable<'a>(pub &'a RwLock<FunctionTable>);

impl SharedFunctionTable<'_> {
    fn read(&self) -> std::sync::RwLockReadGuard<'_, FunctionTable> {
        self.0
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, FunctionTable> {
        self.0
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn probe_or_load<T>(
        &self,
        probe: impl FnOnce(&FunctionTable) -> CacheProbe<T>,
        load: impl FnOnce(&mut FunctionTable) -> T,
    ) -> T {
        {
            let table = self.read();
            if let CacheProbe::Hit(value) = probe(&table) {
                return value;
            }
        }
        let mut table = self.write();
        load(&mut table)
    }

    /// Same read-then-write split as the [`ExternalSignatures`] methods,
    /// for editor autocompletion (not on that trait).
    pub fn list_members(&self, type_name: &str) -> Vec<Member> {
        self.probe_or_load(
            |table| table.list_members_cached(type_name),
            |table| table.list_members(type_name),
        )
    }
}

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

#[cfg(test)]
#[path = "shared_tests.rs"]
mod tests;
