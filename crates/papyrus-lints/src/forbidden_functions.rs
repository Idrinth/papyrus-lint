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
/// identifiers are case-insensitive). Every entry in
/// `rules/forbidden-functions.yaml` currently has a unique function name,
/// so this has no false matches in practice.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = match Lexer::new(source).tokenize() {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };

    let mut diagnostics = Vec::new();
    for window in tokens.windows(2) {
        let TokenKind::Identifier(name) = &window[0].kind else {
            continue;
        };
        if !matches!(window[1].kind, TokenKind::LParen) {
            continue;
        }
        let Some(rule) = find_rule(name) else {
            continue;
        };
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
}
