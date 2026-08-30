//! Fallback knowledge of native (engine-defined) Papyrus scripts that are
//! always referenced by their literal type name (e.g. `Game.GetPlayer()`,
//! `Utility.Wait(1.0)`) rather than through a variable or property, for
//! [`crate::function_table::FunctionTable::script_exists`].
//!
//! These native singleton scripts are shipped compiled with the game, so a
//! typical mod project carries no `.psc` source for them under its own
//! `scripts/source`/`source/scripts` — without this table, a perfectly
//! ordinary call like `Game.GetPlayer()` or `Utility.Wait(1.0)` would be
//! flagged by the "Unresolved script reference" lint as calling a script
//! that doesn't exist.
//!
//! The `NATIVE_GLOBALS` table below is compiled from
//! `rules/native-globals.yaml` by `build.rs` at build time (like
//! [`crate::native_types`]), so extending the list doesn't need a code
//! change. It is deliberately not exhaustive: a script this table doesn't
//! know about (including one the linter simply has no data for, e.g. a
//! SKSE/F4SE plugin or community function library) is resolved by looking
//! it up under the project instead, same as any other script name.
include!(concat!(env!("OUT_DIR"), "/native_globals_data.rs"));

/// Whether `name_lower` is a known native singleton script, always called
/// through its literal name. `name_lower` must already be lowercased
/// (callers already work in lowercase for case-insensitive matching).
pub fn is_known(name_lower: &str) -> bool {
    NATIVE_GLOBALS.contains(&name_lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_common_native_singleton_scripts() {
        assert!(is_known("game"));
        assert!(is_known("utility"));
        assert!(is_known("debug"));
    }

    #[test]
    fn is_case_sensitive_to_its_already_lowercased_input() {
        // Callers are expected to lowercase before calling, same as
        // `native_types::parent_of`; this only documents that expectation.
        assert!(!is_known("Game"));
    }

    #[test]
    fn returns_false_for_an_unknown_script() {
        assert!(!is_known("somemodsquestscript"));
    }
}
