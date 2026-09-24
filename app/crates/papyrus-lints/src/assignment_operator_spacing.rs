//! Requires exactly one space on either side of the assignment (`=`, `+=`,
//! `-=`, `*=`, `/=`, `%=`) operators.

use crate::binary_operator_spacing::{self, BinaryOperatorSpacing};
use crate::Diagnostic;
use papyrus_parser::token::TokenKind;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "assignment-operator-spacing";

struct Lint;

impl BinaryOperatorSpacing for Lint {
    const RULE: &'static str = RULE;

    fn operator_text(kind: &TokenKind) -> Option<&'static str> {
        match kind {
            TokenKind::Assign => Some("="),
            TokenKind::PlusAssign => Some("+="),
            TokenKind::MinusAssign => Some("-="),
            TokenKind::StarAssign => Some("*="),
            TokenKind::SlashAssign => Some("/="),
            TokenKind::PercentAssign => Some("%="),
            _ => None,
        }
    }
}

pub fn visitor() -> crate::visitor::LintVisitor {
    binary_operator_spacing::visitor::<Lint>()
}

/// Checks for an assignment operator not surrounded by exactly one space on
/// a side that shares its physical line with the operator (a side whose
/// whitespace run reaches a newline — the operator opens or closes a
/// statement continued across lines — is never flagged on that side).
/// Operators on a line protected by a CreationKit fragment-code wrapper
/// (see [`crate::fragment_code`]) are never flagged. `==`, `!=`, `>=`, and `<=`
/// are never matched here, since the lexer tokenizes those as their own,
/// separate token kinds (see [`crate::operator_spacing`] instead).
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

/// Normalizes the whitespace on either side of every assignment operator to
/// exactly one space, applying the same same-line rule (and fragment-code
/// exemption) as [`check`]. A gap that reaches a newline is left exactly
/// as-is, so a statement continued across physical lines keeps its own
/// line breaks.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    binary_operator_spacing::repair::<Lint>(source, ast, tokens, config)
}

#[cfg(test)]
#[path = "assignment_operator_spacing_tests.rs"]
mod tests;
