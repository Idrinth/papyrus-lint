//! Flags, as a `[warning]`, a call to an Actor Value function (`GetActorValue`,
//! `SetActorValue`, `ModActorValue`, and the rest of that family — see
//! [`ACTOR_VALUE_FUNCTIONS`]) whose Actor Value name argument doesn't match
//! one of Skyrim's built-in Actor Values, listed in
//! `rules/actor-values.yaml`.
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
//! `rules/actor-values.yaml`, which would otherwise be misreported here.

use papyrus_parser::token::TokenKind;

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

/// Checks `source` for calls to an Actor Value function whose Actor Value
/// argument isn't one of Skyrim's built-in Actor Values.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = match papyrus_parser::tokenize(source) {
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
        if !is_actor_value_function(name) {
            continue;
        }
        let Some(TokenKind::StringLiteral(value)) = tokens.get(i + 2).map(|t| &t.kind) else {
            continue;
        };
        if is_known_actor_value(value) {
            continue;
        }
        diagnostics.push(Diagnostic {
            line: window[0].line,
            column: window[0].col,
            message: format!(
                "[warning] {name}(\"{value}\", ...): \"{value}\" is not a recognized Actor Value"
            ),
            rule: RULE,
        });
    }
    diagnostics
}

fn is_actor_value_function(name: &str) -> bool {
    ACTOR_VALUE_FUNCTIONS
        .iter()
        .any(|function| function.eq_ignore_ascii_case(name))
}

fn is_known_actor_value(value: &str) -> bool {
    ACTOR_VALUES
        .iter()
        .any(|actor_value| actor_value.eq_ignore_ascii_case(value))
}

#[cfg(test)]
#[path = "actor_value_tests.rs"]
mod tests;
