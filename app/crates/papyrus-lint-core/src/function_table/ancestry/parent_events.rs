//! Whether `Parent.Event()` on an extending script would run real setup.

use super::{parent_cache_key, CacheProbe, FunctionTable};
use crate::script_functions::{FunctionSignature, ScriptFunctions};

impl FunctionTable {
    /// Whether a script that `Extends` `type_name` must call
    /// `Parent.event_name()` when overriding that event.
    ///
    /// Stops at the nearest ancestor `Event` of that name. An empty (or
    /// `Return`-only) body, a `Native` event, or an event on a `Native`
    /// script is `Some(false)`: calling `Parent` would not run project
    /// setup, and a non-empty grandparent hidden behind that empty
    /// override is not reached either. `Some(true)` is a non-empty event
    /// on a project script. `None` is a missing script or a circular
    /// `Extends` before any such event is found. Both names are matched
    /// case-insensitively.
    pub fn parent_event_needs_call(&mut self, type_name: &str, event_name: &str) -> Option<bool> {
        let event_key = event_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                return None;
            }
            self.ensure_loaded(&name);
            let script = self.scripts.get(&name).and_then(Option::as_ref)?;
            if let Some(needs_call) = declared_event_needs_call(script, &event_key) {
                return Some(needs_call);
            }
            current = parent_cache_key(script);
            visited.push(name);
        }

        Some(false)
    }

    /// [`Self::parent_event_needs_call`] from scripts already cached.
    /// [`CacheProbe::Miss`] means some ancestor still needs
    /// [`FunctionTable::ensure_loaded`].
    pub(in crate::function_table) fn parent_event_needs_call_cached(
        &self,
        type_name: &str,
        event_name: &str,
    ) -> CacheProbe<Option<bool>> {
        let event_key = event_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                return CacheProbe::Hit(None);
            }
            let script = match self.get_cached(&name) {
                None => return CacheProbe::Miss,
                Some(None) => return CacheProbe::Hit(None),
                Some(Some(script)) => script,
            };
            if let Some(needs_call) = declared_event_needs_call(script, &event_key) {
                return CacheProbe::Hit(Some(needs_call));
            }
            current = parent_cache_key(script);
            visited.push(name);
        }

        CacheProbe::Hit(Some(false))
    }
}

/// `Some` when this script itself declares `event_key` as an event. The
/// bool is whether a child override must call `Parent`.
fn declared_event_needs_call(script: &ScriptFunctions, event_key: &str) -> Option<bool> {
    let signature = event_signature(script, event_key)?;
    Some(!script.is_native && !signature.is_native && !script.noop_events.contains(event_key))
}

fn event_signature<'a>(
    script: &'a ScriptFunctions,
    event_key: &str,
) -> Option<&'a FunctionSignature> {
    script
        .functions
        .get(event_key)
        .filter(|signature| signature.is_event)
}
