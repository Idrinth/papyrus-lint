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
//! The `NATIVE_GLOBALS` tables below are compiled from the bundled Creation
//! Kit / script-extender archives by `build.rs` at build time, so extending
//! the list doesn't need a code change. It is deliberately not exhaustive:
//! a script this table doesn't know about (including one the linter simply
//! has no data for, e.g. a SKSE/F4SE plugin or community function library)
//! is resolved by looking it up under the project instead, same as any
//! other script name.
include!(concat!(env!("OUT_DIR"), "/native_globals_data.rs"));

fn globals_for(game: &str) -> &'static [&'static str] {
    match game {
        "fallout4" => FALLOUT4_NATIVE_GLOBALS,
        "skyrim" => SKYRIM_NATIVE_GLOBALS,
        _ => panic!("unsupported game {game} provided"),
    }
}

/// Whether `name_lower` is a known native singleton script for `game`.
/// `name_lower` must already be lowercased.
pub fn is_known_for(game: &str, name_lower: &str) -> bool {
    globals_for(game).contains(&name_lower)
}
