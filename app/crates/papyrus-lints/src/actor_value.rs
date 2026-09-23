//! Flags, as a `[warning]`, a call to an Actor Value function (`GetActorValue`,
//! `SetActorValue`, `ModActorValue`, and the rest of that family — see
//! [`ACTOR_VALUE_FUNCTIONS`]) whose Actor Value name argument doesn't match
//! one of the target game's built-in Actor Values, listed in
//! `shared/rules/data/{skyrim,fallout4}/actor-values.yaml`.
//!
//! Like [`crate::forbidden_functions`], this works on lexer tokens rather
//! than the parsed AST, so it still runs on scripts that don't parse
//! cleanly, and matches a call by function name alone (case-insensitively),
//! regardless of its receiver, since the lexer has no type resolution to
//! confirm the receiver is actually an `Actor` (or subtype). Only a call
//! whose Actor Value argument is a plain string literal is checked; one
//! built from a variable or any other expression is left unflagged rather
//! than guessed at, since its value can't be determined statically.
//!
//! Disabled by default: a project's own plugin can define additional,
//! custom Actor Values that have no way to appear in
//! `shared/rules/data/{skyrim,fallout4}/actor-values.yaml`, which would otherwise be misreported here.

use papyrus_parser::token::{Token, TokenKind};

use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;

include!(concat!(env!("OUT_DIR"), "/actor_values_data.rs"));

/// The Papyrus functions that take an Actor Value name as their first
/// argument, per the CreationKit wiki's "Actor Value" page.
const ACTOR_VALUE_FUNCTIONS: &[&str] = &[
    "DamageActorValue",
    "DamageAV",
    "ForceAV",
    "GetActorValue",
    "GetActorValuePercentage",
    "GetAV",
    "GetAVPercent",
    "GetBaseActorValue",
    "GetBaseAV",
    "ModActorValue",
    "ModAV",
    "RestoreActorValue",
    "RestoreAV",
    "SetActorValue",
    "SetAV",
    "GetActorValueMax",
    "GetAVMax",
];

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unknown-actor-value";

#[derive(Default)]
struct Collect {
    store: Store,
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
        ctx: &mut VisitCtx<'_>,
    ) {
        let TokenKind::Identifier(name) = &token.kind else {
            return;
        };
        if !matches!(tokens.get(index + 1).map(|token| &token.kind), Some(TokenKind::LParen)) {
            return;
        }
        if !is_actor_value_function(name) {
            return;
        }
        let Some(TokenKind::StringLiteral(value)) = tokens.get(index + 2).map(|token| &token.kind)
        else {
            return;
        };
        if is_known_actor_value(value, ctx.config.game.as_str()) {
            return;
        }
        self.store.emit(
            token.line,
            token.col,
            format!(
                "[warning] {name}(\"{value}\", ...): \"{value}\" is not a recognized Actor Value"
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Checks `source` for calls to an Actor Value function whose Actor Value
/// argument isn't one of the target game's built-in Actor Values.
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

fn is_actor_value_function(name: &str) -> bool {
    ACTOR_VALUE_FUNCTIONS
        .iter()
        .any(|function| function.eq_ignore_ascii_case(name))
}

fn is_known_actor_value(value: &str, game: &str) -> bool {
    actor_values_for(game)
        .iter()
        .any(|actor_value| actor_value.eq_ignore_ascii_case(value))
}

#[cfg(test)]
#[path = "actor_value_tests.rs"]
mod tests;
