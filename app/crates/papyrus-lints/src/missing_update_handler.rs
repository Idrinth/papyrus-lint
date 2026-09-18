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
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, ast, config, external);

    let Some(tokens) = tokens else {
        return Vec::new();
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
        if declares_event(tokens, rule.event) {
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
#[path = "missing_update_handler_tests.rs"]
mod tests;
