//! Flags, as a `[warning]`, a call to `RegisterForUpdate`,
//! `RegisterForSingleUpdate`, `RegisterForUpdateGameTime`, or
//! `RegisterForSingleUpdateGameTime` in a script that declares no matching
//! `Event` (`OnUpdate` or `OnUpdateGameTime`, per
//! `rules/update-event-handlers.yaml`) anywhere in it, since the engine
//! then has nothing to call once the registered timer fires and the
//! registration has no effect.
//!
//! Register-function/Event-name pairs are compiled into the
//! `UPDATE_EVENT_PAIRS` array below by `build.rs` at build time, so this
//! never parses YAML at runtime. Like the other lints in this crate, it
//! works on lexer tokens rather than the parsed AST, so it still runs on
//! scripts that don't parse cleanly, and matches a `RegisterFor*` call by
//! function name alone (case-insensitively), regardless of its receiver,
//! since the lexer has no type resolution to confirm it's called on
//! `Self`/an `ObjectReference`/an `Actor`. A matching `Event` is
//! recognized anywhere in the script, in any `State` block, not just the
//! empty state, since a call made from one state can still fall back to
//! the empty state's own handler.
//!
//! Disabled by default: a script's own `RegisterFor*` call is only half of
//! this pair when its script `Extends` another one that declares the
//! matching `Event` itself, and this lint has no way to resolve that
//! ancestor from a single script's source alone, which would otherwise be
//! misreported here.

use papyrus_parser::token::{Keyword, Token, TokenKind};

use crate::Diagnostic;

pub struct UpdateEventPairRule {
    pub register: &'static str,
    pub event: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/update_event_pairs_data.rs"));

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "missing-update-handler";

/// Checks `source` for a `RegisterFor*` call with no matching `Event`
/// declared anywhere in the same script.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = match papyrus_parser::tokenize(source) {
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
        if declares_event(&tokens, rule.event) {
            continue;
        }
        diagnostics.push(Diagnostic {
            line: window[0].line,
            column: window[0].col,
            message: format!(
                "[warning] {name}(...) registers for updates, but this script declares no \
                 Event {}() to receive them; the registration has no effect",
                rule.event
            ),
            rule: RULE,
        });
    }
    diagnostics
}

fn find_rule(name: &str) -> Option<&'static UpdateEventPairRule> {
    UPDATE_EVENT_PAIRS
        .iter()
        .find(|rule| rule.register.eq_ignore_ascii_case(name))
}

/// Whether `tokens` declares an `Event` named `event` (case-insensitively)
/// anywhere, regardless of which `State` block (if any) it's declared in.
fn declares_event(tokens: &[Token], event: &str) -> bool {
    tokens.windows(2).any(|window| {
        matches!(window[0].kind, TokenKind::Keyword(Keyword::Event))
            && matches!(&window[1].kind, TokenKind::Identifier(name) if name.eq_ignore_ascii_case(event))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_pairs_are_loaded_from_yaml() {
        assert!(!UPDATE_EVENT_PAIRS.is_empty());
        assert!(UPDATE_EVENT_PAIRS
            .iter()
            .any(|pair| pair.register == "RegisterForUpdate" && pair.event == "OnUpdate"));
    }

    #[test]
    fn does_not_flag_register_for_update_with_a_matching_event() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(7.0)\nEndFunction\n\nEvent OnUpdate()\nEndEvent\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_register_for_update_with_no_matching_event() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(7.0)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("RegisterForUpdate"));
        assert!(diagnostics[0].message.contains("OnUpdate"));
    }

    #[test]
    fn flags_register_for_single_update_with_no_matching_event() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForSingleUpdate(1.0)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("RegisterForSingleUpdate"));
    }

    #[test]
    fn register_for_single_update_also_accepts_a_plain_on_update_handler() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForSingleUpdate(1.0)\nEndFunction\n\nEvent OnUpdate()\nEndEvent\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_register_for_update_game_time_with_no_matching_event() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForUpdateGameTime(1.0)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("OnUpdateGameTime"));
    }

    #[test]
    fn does_not_flag_register_for_update_game_time_with_its_own_event() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForUpdateGameTime(1.0)\nEndFunction\n\nEvent OnUpdateGameTime()\nEndEvent\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn on_update_game_time_does_not_satisfy_a_plain_register_for_update() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(1.0)\nEndFunction\n\nEvent OnUpdateGameTime()\nEndEvent\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("RegisterForUpdate"));
    }

    #[test]
    fn flags_a_call_on_an_arbitrary_receiver() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Start(ObjectReference akRef)\n    akRef.RegisterForUpdate(1.0)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn matches_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    registerforupdate(1.0)\nEndFunction\n\nEvent onupdate()\nEndEvent\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn recognizes_an_event_declared_inside_a_state() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(1.0)\nEndFunction\n\nState Active\n    Event OnUpdate()\n    EndEvent\nEndState\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_unrelated_calls() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    Debug.MessageBox(\"hi\")\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(\"unterminated\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_identifiers_that_are_not_calls() {
        let diagnostics = check("ScriptName Example\n\nInt RegisterForUpdate = 1\n");
        assert!(diagnostics.is_empty());
    }
}
