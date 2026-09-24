//! Requires exactly one space on either side of the logical (`&&`, `||`)
//! and comparison (`==`, `!=`, `>`, `<`, `>=`, `<=`) operators.

use crate::binary_operator_spacing::{self, BinaryOperatorSpacing};
use crate::Diagnostic;
use papyrus_parser::token::TokenKind;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "operator-spacing";

struct Lint;

impl BinaryOperatorSpacing for Lint {
    const RULE: &'static str = RULE;

    fn operator_text(kind: &TokenKind) -> Option<&'static str> {
        match kind {
            TokenKind::AndAnd => Some("&&"),
            TokenKind::OrOr => Some("||"),
            TokenKind::Eq => Some("=="),
            TokenKind::NotEq => Some("!="),
            TokenKind::GtEq => Some(">="),
            TokenKind::LtEq => Some("<="),
            TokenKind::Gt => Some(">"),
            TokenKind::Lt => Some("<"),
            _ => None,
        }
    }
}

pub fn visitor() -> crate::visitor::LintVisitor {
    binary_operator_spacing::visitor::<Lint>()
}

/// Checks for a logical/comparison operator not surrounded by exactly one
/// space on a side that shares its physical line with the operator (a side
/// whose whitespace run reaches a newline — the operator opens or closes a
/// statement continued across lines — is never flagged on that side).
/// Operators on a line protected by a CreationKit fragment-code wrapper
/// (see [`crate::fragment_code`]) are never flagged.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    binary_operator_spacing::check::<Lint>(source, ast, tokens, config, external)
}

/// Normalizes the whitespace on either side of every logical/comparison
/// operator to exactly one space, applying the same same-line rule (and
/// fragment-code exemption) as [`check`]. A gap that reaches a newline is
/// left exactly as-is, so a statement continued across physical lines
/// keeps its own line breaks.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    binary_operator_spacing::repair::<Lint>(source, ast, tokens, config)
}

#[cfg(test)]
#[path = "operator_spacing_tests.rs"]
mod tests;
