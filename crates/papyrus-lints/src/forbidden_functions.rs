//! Flags calls to functions listed in `rules/forbidden-functions.yaml`
//! (e.g. functions with known performance or reliability pitfalls).
//!
//! Rules are compiled into the `FORBIDDEN_FUNCTIONS` array below by
//! `build.rs` at build time, so this never parses YAML at runtime. Like
//! the other lints in this crate, it works on tokens rather than the
//! parsed AST, so it still runs on scripts that don't parse cleanly.

use crate::Diagnostic;
use papyrus_parser::lexer::Lexer;
use papyrus_parser::token::TokenKind;

pub struct ForbiddenFunctionRule {
    pub script: &'static str,
    pub function: &'static str,
    pub level: &'static str,
    pub message: &'static str,
    /// Whether `script` is a native singleton (e.g. `Game`, `Utility`)
    /// always called through its literal script name, rather than a base
    /// type (e.g. `ObjectReference`, `ScriptObject`) called through a
    /// variable of some subclass. See `check` for how this is used.
    pub global: bool,
}

include!(concat!(env!("OUT_DIR"), "/forbidden_functions_data.rs"));

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "forbidden-functions";

/// Checks `source` for calls to forbidden/discouraged functions.
///
/// A call site is any identifier immediately followed by `(`. The lexer
/// has no type/symbol resolution, so — like the receiver in
/// `akRef.GetLinkedRef()` — a call's qualifier can't generally be
/// resolved back to the script that declares the function; matching is
/// therefore done by function name alone, case-insensitively (Papyrus
/// identifiers are case-insensitive).
///
/// The exception is a rule whose `script` is a native singleton
/// (`global: true` in the YAML, e.g. `Utility`) rather than a base type
/// used through a variable: those scripts are never subclassed, so a
/// qualified call to one of their functions is only a real match when the
/// qualifier is literally that script's name. This is what keeps
/// `Utility.Wait()` flagged while `MyScript.Wait()` (a same-named function
/// on an unrelated script) is not.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = match Lexer::new(source).tokenize() {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };

    let mut diagnostics = Vec::new();
    for (i, window) in tokens.windows(2).enumerate() {
        let TokenKind::Identifier(name) = &window[0].kind else {
            continue;
        };
        if !matches!(window[1].kind, TokenKind::LParen) {
            continue;
        }
        let Some(rule) = find_rule(name) else {
            continue;
        };
        if rule.global && !qualifier_matches(&tokens, i, rule.script) {
            continue;
        }
        diagnostics.push(Diagnostic {
            line: window[0].line,
            column: window[0].col,
            message: format!(
                "[{}] {}.{}: {}",
                rule.level, rule.script, rule.function, rule.message
            ),
            rule: RULE,
        });
    }
    diagnostics
}

/// Whether the call at `tokens[call_index]` is qualified with `script`
/// (case-insensitively), i.e. preceded by `script.`.
fn qualifier_matches(
    tokens: &[papyrus_parser::token::Token],
    call_index: usize,
    script: &str,
) -> bool {
    if call_index < 2 {
        return false;
    }
    if !matches!(tokens[call_index - 1].kind, TokenKind::Dot) {
        return false;
    }
    let TokenKind::Identifier(qualifier) = &tokens[call_index - 2].kind else {
        return false;
    };
    qualifier.eq_ignore_ascii_case(script)
}

fn find_rule(name: &str) -> Option<&'static ForbiddenFunctionRule> {
    FORBIDDEN_FUNCTIONS
        .iter()
        .find(|rule| rule.function.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_rules_are_loaded_from_yaml() {
        assert_eq!(FORBIDDEN_FUNCTIONS.len(), 12);
        assert!(FORBIDDEN_FUNCTIONS
            .iter()
            .any(|r| r.script == "Game" && r.function == "GetPlayer" && r.level == "error"));
    }

    #[test]
    fn flags_qualified_call() {
        let diagnostics =
            check("ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer()\nEndFunction\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0]
            .message
            .starts_with("[error] Game.GetPlayer:"));
    }

    #[test]
    fn flags_unqualified_call() {
        let diagnostics = check(
            "ScriptName Example extends ObjectReference\n\nFunction DoThing()\n    GetLinkedRef()\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("GetLinkedRef"));
    }

    #[test]
    fn flags_call_on_arbitrary_receiver() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(ObjectReference akRef)\n    akRef.RegisterForUpdate(1.0)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("RegisterForUpdate"));
    }

    #[test]
    fn does_not_flag_unrelated_calls() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Debug.MessageBox(\"hi\")\n    self.DoOtherThing()\nEndFunction\n\nFunction DoOtherThing()\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_identifiers_that_are_not_calls() {
        let diagnostics = check("ScriptName Example\n\nInt GetPlayer = 1\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics =
            check("ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer(\"unterminated\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_global_singleton_call_qualified_by_its_own_name() {
        let diagnostics =
            check("ScriptName Example\n\nFunction DoThing()\n    Utility.Wait(1.0)\nEndFunction\n");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Utility.Wait"));
    }

    #[test]
    fn does_not_flag_same_named_function_on_a_different_script() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(MyScript akOther)\n    akOther.Wait(1.0)\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_unqualified_call_to_a_global_singleton_function() {
        let diagnostics =
            check("ScriptName Example\n\nFunction DoThing()\n    Wait(1.0)\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }
}
