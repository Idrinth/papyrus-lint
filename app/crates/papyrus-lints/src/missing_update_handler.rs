//! Flags, as a `[warning]`, a call to `RegisterForUpdate`,
//! `RegisterForSingleUpdate`, `RegisterForUpdateGameTime`, or
//! `RegisterForSingleUpdateGameTime` in a script that declares no matching
//! `Event` (`OnUpdate` or `OnUpdateGameTime`, per
//! `shared/rules/data/skyrim/update-event-handlers.yaml`) anywhere in it, since the engine
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

use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;

pub struct UpdateEventPairRule {
    pub register: &'static str,
    pub event: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/update_event_pairs_data.rs"));

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "missing-update-handler";

#[derive(Default)]
struct Collect {
    store: Store,
    events: Vec<String>,
    calls: Vec<(String, usize, usize, &'static UpdateEventPairRule)>,
}

impl TokenLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_token(
        &mut self,
        token: &Token,
        index: usize,
        tokens: &[Token],
        _ctx: &mut VisitCtx<'_>,
    ) {
        if matches!(token.kind, TokenKind::Keyword(Keyword::Event)) {
            if let Some(TokenKind::Identifier(name)) = tokens.get(index + 1).map(|token| &token.kind)
            {
                self.events.push(name.clone());
            }
        }
        let TokenKind::Identifier(name) = &token.kind else {
            return;
        };
        if !matches!(tokens.get(index + 1).map(|token| &token.kind), Some(TokenKind::LParen)) {
            return;
        }
        let Some(rule) = find_rule(name) else {
            return;
        };
        self.calls
            .push((name.clone(), token.line, token.col, rule));
    }

    fn finish(&mut self, _ctx: &mut VisitCtx<'_>) {
        for (name, line, column, rule) in &self.calls {
            if self
                .events
                .iter()
                .any(|event| event.eq_ignore_ascii_case(rule.event))
            {
                continue;
            }
            self.store.emit(
                *line,
                *column,
                format!(
                    "[warning] {name}(...) registers for updates, but this script declares no \
                     Event {}() to receive them; the registration has no effect",
                    rule.event
                ),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Checks `source` for a `RegisterFor*` call with no matching `Event`
/// declared anywhere in the same script.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

fn find_rule(name: &str) -> Option<&'static UpdateEventPairRule> {
    UPDATE_EVENT_PAIRS
        .iter()
        .find(|rule| rule.register.eq_ignore_ascii_case(name))
}

#[cfg(test)]
#[path = "missing_update_handler_tests.rs"]
mod tests;
