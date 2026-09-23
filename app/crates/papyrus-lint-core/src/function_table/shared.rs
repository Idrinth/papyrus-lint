//! A thread-safe [`papyrus_lints::ExternalSignatures`] adapter over a
//! [`super::FunctionTable`] shared by multiple lint workers.

use std::sync::RwLock;

use super::{CacheProbe, FunctionTable, Member};

mod external_signatures;

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
/// drift from that impl's policy (e.g. `type_exists`'s primitive-type
/// handling).
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

#[cfg(test)]
#[path = "shared_tests.rs"]
mod tests;
