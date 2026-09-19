//! Flags whitespace that interrupts a property or method access chain,
//! e.g. `SomeProperty . DoThing()` instead of `SomeProperty.DoThing()`.

use crate::{fragment_code, Diagnostic};
use papyrus_parser::token::TokenKind;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "chain-whitespace";

pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::from_tokens(lint_issues)
}

const WHITESPACE: [u8; 2] = *b" \t";

/// Checks for a `.` member/method access whose adjacent character, on
/// either side and on the same line, is a space or tab, since that
/// whitespace interrupts the chain for no benefit. Always reported as a
/// `[warning]`. A dot inside a `Float` literal (e.g. `1.5`) is lexed as part
/// of the number itself and never reaches this check. Dots on a line
/// protected by a CreationKit fragment-code wrapper (see [`fragment_code`])
/// are never flagged.
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
        if token.kind != TokenKind::Dot || protected[token.line] {
            continue;
        }

        let offset = line_starts[token.line - 1] + token.col - 1;

        let before = offset
            .checked_sub(1)
            .and_then(|index| bytes.get(index))
            .copied();
        if before.is_some_and(|byte| WHITESPACE.contains(&byte)) {
            diagnostics.push(Diagnostic {
                line: token.line,
                column: token.col,
                message: "[warning] Whitespace before '.' interrupts property/method chaining"
                    .to_string(),
                rule: RULE,
            });
        }

        let after = bytes.get(offset + 1).copied();
        if after.is_some_and(|byte| WHITESPACE.contains(&byte)) {
            diagnostics.push(Diagnostic {
                line: token.line,
                column: token.col,
                message: "[warning] Whitespace after '.' interrupts property/method chaining"
                    .to_string(),
                rule: RULE,
            });
        }
    }
    diagnostics
}

/// Removes whitespace immediately before/after a `.` member/method access,
/// closing the same gaps [`check`] flags (a dot inside a `Float` literal, or
/// on a line protected by a CreationKit fragment-code wrapper, is left
/// alone). Only the contiguous run of spaces/tabs touching the dot itself is
/// removed; anything past a newline (a chain continued onto another
/// physical line) is untouched, matching what [`check`] considers "the same
/// line".
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

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for token in tokens {
        if token.kind != TokenKind::Dot || protected[token.line] {
            continue;
        }
        let offset = line_starts[token.line - 1] + token.col - 1;

        let mut start = offset;
        while start > 0 && WHITESPACE.contains(&bytes[start - 1]) {
            start -= 1;
        }
        if start < offset {
            ranges.push((start, offset));
        }

        let mut end = offset + 1;
        while end < bytes.len() && WHITESPACE.contains(&bytes[end]) {
            end += 1;
        }
        if end > offset + 1 {
            ranges.push((offset + 1, end));
        }
    }

    if ranges.is_empty() {
        return source.to_string();
    }
    ranges.sort_unstable();

    let mut repaired = String::with_capacity(source.len());
    let mut previous = 0;
    for (start, end) in ranges {
        let start = start.max(previous);
        if start >= end {
            continue;
        }
        repaired.push_str(&source[previous..start]);
        previous = end;
    }
    repaired.push_str(&source[previous..]);
    repaired
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
#[path = "chain_whitespace_tests.rs"]
mod tests;
