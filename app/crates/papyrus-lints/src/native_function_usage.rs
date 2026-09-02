//! Flags a `Native` function/event declared on a linted script whose
//! (script, function) pair isn't one of the base-game native functions
//! listed in `rules/native-methods.yaml`.
//!
//! A `Native` declaration has no body of its own — its implementation is
//! supplied by the engine (or, for a modder-authored header script, by an
//! SKSE/F4SE plugin DLL). Since `rules/native-methods.yaml` only lists the
//! functions Skyrim's own base-game scripts declare `Native`, a `Native`
//! declaration that doesn't match an entry there is a strong signal the
//! project depends on a native extension rather than anything the base game
//! ships. Disabled by default, since plenty of mods intentionally depend on
//! SKSE/F4SE or another native extension and don't need to be warned about
//! it.
//!
//! Rules are compiled into the `NATIVE_METHODS` array below by `build.rs`
//! at build time, so this never parses YAML at runtime. Unlike most of the
//! other lints in this crate, this works from the parsed AST rather than
//! raw tokens, since it needs each function's declared `Native` flag; a
//! script that doesn't parse cleanly is left unchecked rather than guessed
//! at.

use papyrus_parser::ast::{FunctionDecl, Script};

use crate::Diagnostic;

pub struct NativeMethodRule {
    pub object: &'static str,
    pub function: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/native_methods_data.rs"));

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "native-function-usage";

/// Checks `source` for `Native` functions/events not supplied by the base
/// game, per `NATIVE_METHODS`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    all_functions(&script)
        .filter(|function| function.is_native)
        .filter(|function| !is_base_game_native(&script.name, &function.name))
        .map(|function| Diagnostic {
            line: function.line,
            column: 1,
            message: format!(
                "[warning] Native function '{}.{}' isn't supplied by the base game; it likely requires SKSE/F4SE or another native extension",
                script.name, function.name
            ),
            rule: RULE,
        })
        .collect()
}

/// Iterates every function declared directly on a script, plus every
/// function declared in each of its states.
fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

/// Whether `(script_name, function_name)` matches a base-game native
/// function listed in `rules/native-methods.yaml`, case-insensitively
/// (Papyrus identifiers are case-insensitive).
fn is_base_game_native(script_name: &str, function_name: &str) -> bool {
    NATIVE_METHODS.iter().any(|rule| {
        rule.object.eq_ignore_ascii_case(script_name)
            && rule.function.eq_ignore_ascii_case(function_name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_rules_are_loaded_from_yaml() {
        assert!(!NATIVE_METHODS.is_empty());
        assert!(NATIVE_METHODS
            .iter()
            .any(|rule| rule.object == "Actor" && rule.function == "AddPerk"));
    }

    #[test]
    fn does_not_flag_a_base_game_native_function() {
        let diagnostics = check(
            "ScriptName Actor\n\nFunction AddPerk(Perk akPerk, Bool abForceInform = true) Native\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn matches_the_base_game_function_case_insensitively() {
        let diagnostics = check(
            "ScriptName actor\n\nFunction addperk(Perk akPerk, Bool abForceInform = true) Native\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_native_function_not_supplied_by_the_base_game() {
        let diagnostics = check(
            "ScriptName MyNativeLib\n\nFunction DoSomethingNative(Int aiValue) Global Native\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert!(diagnostics[0]
            .message
            .contains("MyNativeLib.DoSomethingNative"));
        assert!(diagnostics[0].message.starts_with("[warning]"));
    }

    #[test]
    fn flags_a_native_function_declared_inside_a_state() {
        let diagnostics = check(
            "ScriptName MyNativeLib\n\nState Busy\n    Function DoSomethingNative(Int aiValue) Global Native\nEndState\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn does_not_flag_a_function_with_a_body() {
        let diagnostics = check(
            "ScriptName MyScript\n\nFunction DoThing()\n    Debug.Trace(\"hi\")\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_same_named_function_declared_on_an_unrelated_script() {
        let diagnostics =
            check("ScriptName MyActorHelper\n\nFunction AddPerk(Perk akPerk) Native\n");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("MyActorHelper.AddPerk"));
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName Example\n\nFunction DoThing(\n");
        assert!(diagnostics.is_empty());
    }
}
