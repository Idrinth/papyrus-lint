//! Flags calls to functions listed in `rules/slow-functions.yaml` that
//! have a faster equivalent available, and suggests that replacement.
//!
//! Rules are compiled into the `SLOW_FUNCTIONS` array below by `build.rs`
//! at build time, so this never parses YAML at runtime. Like the other
//! lints in this crate, it works on tokens rather than the parsed AST, so
//! it still runs on scripts that don't parse cleanly.

use crate::Diagnostic;
use papyrus_parser::lexer::Lexer;
use papyrus_parser::token::TokenKind;

pub struct SlowFunctionRule {
    pub object: &'static str,
    pub function: &'static str,
    pub replacement: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/slow_functions_data.rs"));

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "slow-functions";

/// Checks `source` for calls to functions with a faster equivalent.
///
/// A call site is any identifier immediately followed by `(`. The lexer
/// has no type/symbol resolution, so — like the receiver in
/// `akGlobal.GetValueInt()` — a call's qualifier can't generally be
/// resolved back to the script that declares the function; matching is
/// therefore done by function name alone, case-insensitively (Papyrus
/// identifiers are case-insensitive). Every entry in
/// `rules/slow-functions.yaml` currently has a unique function name, so
/// this has no false matches in practice.
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
                "{}.{} is slower than necessary; use `{}` instead",
                rule.object, rule.function, rule.replacement
            ),
            rule: RULE,
        });
    }
    diagnostics
}

fn find_rule(name: &str) -> Option<&'static SlowFunctionRule> {
    SLOW_FUNCTIONS
        .iter()
        .find(|rule| rule.function.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_rules_are_loaded_from_yaml() {
        assert_eq!(SLOW_FUNCTIONS.len(), 2);
        assert!(SLOW_FUNCTIONS
            .iter()
            .any(|r| r.object == "GlobalVariable" && r.function == "GetValueInt"));
    }

    #[test]
    fn flags_qualified_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt()\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0]
            .message
            .contains("GlobalVariable.GetValueInt"));
        assert!(diagnostics[0].message.contains("GetValue() As Int"));
    }

    #[test]
    fn flags_call_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.setvalueint(1)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("SetValueInt"));
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
        let diagnostics = check("ScriptName Example\n\nInt GetValueInt = 1\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    akGlobal.GetValueInt(\"unterminated\n",
        );
        assert!(diagnostics.is_empty());
    }
}
