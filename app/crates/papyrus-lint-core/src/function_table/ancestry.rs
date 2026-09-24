//! `Extends`-chain lookups against a [`super::FunctionTable`], split by lookup concern.

use super::{CacheProbe, FunctionTable};
#[cfg(test)]
use crate::script_functions::Member;
use crate::script_functions::ScriptFunctions;

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

mod cached;
mod events;
mod members;
mod states;

pub(in crate::function_table) use events::ResolvedEvents;

impl FunctionTable {
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
}

#[cfg(test)]
#[path = "ancestry_tests.rs"]
mod tests;
