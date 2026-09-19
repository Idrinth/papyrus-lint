//! Requires exactly one space on either side of the assignment (`=`, `+=`,
//! `-=`, `*=`, `/=`, `%=`) operators.

use crate::{fragment_code, Diagnostic};
use papyrus_parser::token::TokenKind;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "assignment-operator-spacing";

pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::from_tokens(lint_issues)
}

/// The source text of a token this lint cares about, or `None` for every
/// other token kind.
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

/// Checks for an assignment operator not surrounded by exactly one space on
/// a side that shares its physical line with the operator (a side whose
/// whitespace run reaches a newline — the operator opens or closes a
/// statement continued across lines — is never flagged on that side).
/// Operators on a line protected by a CreationKit fragment-code wrapper
/// (see [`fragment_code`]) are never flagged. `==`, `!=`, `>=`, and `<=`
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
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

fn lint_issues(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut dyn crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (ast, config, external);

    let Some(tokens) = tokens else {
        return Vec::new();
    };
    let protected = fragment_code::protected_lines(source);
    let line_starts = line_starts(source);
    let bytes = source.as_bytes();

    let mut diagnostics = Vec::new();
    for token in tokens {
        let Some(text) = operator_text(&token.kind) else {
            continue;
        };
        if protected[token.line] {
            continue;
        }
        let offset = line_starts[token.line - 1] + token.col - 1;
        let end = offset + text.len();

        if let Some((start, len)) = leading_gap(bytes, offset) {
            if !(len == 1 && bytes[start] == b' ') {
                diagnostics.push(Diagnostic {
                    line: token.line,
                    column: token.col,
                    message: format!("[warning] '{text}' must be preceded by exactly one space"),
                    rule: RULE,
                });
            }
        }
        if let Some((start, gend)) = trailing_gap(bytes, end) {
            if !(gend - start == 1 && bytes[start] == b' ') {
                diagnostics.push(Diagnostic {
                    line: token.line,
                    column: token.col,
                    message: format!("[warning] '{text}' must be followed by exactly one space"),
                    rule: RULE,
                });
            }
        }
    }
    diagnostics
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
    let _ = (ast, tokens, config);

    let protected = fragment_code::protected_lines(source);
    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return source.to_string();
    };
    let line_starts = line_starts(source);
    let bytes = source.as_bytes();

    let mut edits: Vec<(usize, usize)> = Vec::new();
    for token in tokens {
        let Some(text) = operator_text(&token.kind) else {
            continue;
        };
        if protected[token.line] {
            continue;
        }
        let offset = line_starts[token.line - 1] + token.col - 1;
        let end = offset + text.len();

        if let Some((start, len)) = leading_gap(bytes, offset) {
            if !(len == 1 && bytes[start] == b' ') {
                edits.push((start, offset));
            }
        }
        if let Some((start, gend)) = trailing_gap(bytes, end) {
            if !(gend - start == 1 && bytes[start] == b' ') {
                edits.push((start, gend));
            }
        }
    }

    if edits.is_empty() {
        return source.to_string();
    }
    edits.sort_unstable();
    edits.dedup();

    let mut repaired = String::with_capacity(source.len());
    let mut previous = 0;
    for (start, end) in edits {
        let start = start.max(previous);
        if start > end {
            continue;
        }
        repaired.push_str(&source[previous..start]);
        repaired.push(' ');
        previous = end;
    }
    repaired.push_str(&source[previous..]);
    repaired
}

/// True for a byte that ends a physical line (a newline, or nothing at
/// all, since `\r` in a `\r\n` ending always sits right before a `\n`).
fn is_line_boundary(byte: Option<u8>) -> bool {
    matches!(byte, None | Some(b'\n') | Some(b'\r'))
}

/// The contiguous run of spaces/tabs immediately before `offset`, as
/// `(start, length)`, unless that run reaches the start of the file or a
/// preceding newline — in which case `offset` opens a statement continued
/// from a previous physical line, which this lint leaves alone, and `None`
/// is returned.
fn leading_gap(bytes: &[u8], offset: usize) -> Option<(usize, usize)> {
    let mut start = offset;
    while start > 0 && (bytes[start - 1] == b' ' || bytes[start - 1] == b'\t') {
        start -= 1;
    }
    if start == 0 || bytes[start - 1] == b'\n' {
        return None;
    }
    Some((start, offset - start))
}

/// The contiguous run of spaces/tabs starting at `offset`, as
/// `(start, end)`, unless that run reaches the end of the file or a
/// following newline — in which case the statement continues onto the
/// next physical line, which this lint leaves alone, and `None` is
/// returned.
fn trailing_gap(bytes: &[u8], offset: usize) -> Option<(usize, usize)> {
    let mut end = offset;
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    if is_line_boundary(bytes.get(end).copied()) {
        return None;
    }
    Some((offset, end))
}

fn line_starts(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        )
        .collect()
}

#[cfg(test)]
#[path = "assignment_operator_spacing_tests.rs"]
mod tests;
