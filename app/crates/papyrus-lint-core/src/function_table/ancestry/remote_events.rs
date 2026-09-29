//! Ancestry lookup for `RegisterForRemoteEvent` registrations.

use super::{parent_cache_key, CacheProbe, FunctionTable};
use crate::script_functions::ScriptFunctions;

impl FunctionTable {
    /// Whether `type_name` or an ancestor it `Extends` registered for the
    /// remote event leaf `event_name` (case-insensitive). An opaque
    /// non-literal `RegisterForRemoteEvent` on any script in the chain
    /// counts as coverage. `None` when the chain cannot be fully resolved
    /// (missing script or circular `Extends`).
    pub fn registers_remote_event(&mut self, type_name: &str, event_name: &str) -> Option<bool> {
        let event_key = event_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());
        let mut saw_any = false;

        while let Some(name) = current {
            if visited.contains(&name) {
                return None;
            }
            self.ensure_loaded(&name);
            let script = self.scripts.get(&name).and_then(Option::as_ref)?;
            saw_any = true;
            if script_covers_remote_event(script, &event_key) {
                return Some(true);
            }
            current = parent_cache_key(script);
            visited.push(name);
        }

        if saw_any {
            Some(false)
        } else {
            None
        }
    }

    /// [`Self::registers_remote_event`] answered only from scripts already
    /// cached. [`CacheProbe::Miss`] means some ancestor still needs
    /// [`FunctionTable::ensure_loaded`].
    pub(in crate::function_table) fn registers_remote_event_cached(
        &self,
        type_name: &str,
        event_name: &str,
    ) -> CacheProbe<Option<bool>> {
        let event_key = event_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());
        let mut saw_any = false;

        while let Some(name) = current {
            if visited.contains(&name) {
                return CacheProbe::Hit(None);
            }
            let script = match self.get_cached(&name) {
                None => return CacheProbe::Miss,
                Some(None) => return CacheProbe::Hit(None),
                Some(Some(script)) => script,
            };
            saw_any = true;
            if script_covers_remote_event(script, &event_key) {
                return CacheProbe::Hit(Some(true));
            }
            current = parent_cache_key(script);
            visited.push(name);
        }

        CacheProbe::Hit(if saw_any { Some(false) } else { None })
    }
}

fn script_covers_remote_event(script: &ScriptFunctions, event_key: &str) -> bool {
    script.opaque_remote_event_registration || script.registered_remote_events.contains(event_key)
}
