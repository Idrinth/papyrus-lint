//! Requires exactly one space between a `!` negation operator and the
//! expression it negates, e.g. `! bReady` instead of `!bReady`, so the
//! negation is easier to spot at a glance.

use crate::{fragment_code, Diagnostic};
use papyrus_parser::token::TokenKind;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "exclamation-spacing";

#[allow(dead_code)] // not dispatched from collect_diagnostics yet
pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::tokens()
}

/// Checks for a `!` negation operator (never `!=`, which the lexer tokenizes
/// separately) whose following characters, on the same line, aren't exactly
/// one plain space. Always reported as a `[warning]`. A `!` on a line
/// protected by a CreationKit fragment-code wrapper (see [`fragment_code`])
/// is never flagged, and neither is a `!` with nothing but a line ending (or
/// end of file) after it — inserting a space there would just be trailing
/// whitespace, which the "Trailing whitespace" fix would strip right back
/// off, so there's nothing this lint can usefully require. A `!` directly
/// followed by another `!` with no space in between (a chained/double
/// negation like `!!bReady`) is left alone too: only the last `!` in such a
/// run needs the trailing space, since spreading the run's own `!`s apart
/// makes the idiom harder to read, not easier.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
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
        if token.kind != TokenKind::Not || protected[token.line] {
            continue;
        }
        let offset = line_starts[token.line - 1] + token.col - 1;
        let (start, end) = whitespace_run(bytes, offset);
        if starts_another_negation(bytes, start, end) {
            continue;
        }
        if !at_end_of_line(bytes, end) && !is_single_space(bytes, start, end) {
            diagnostics.push(Diagnostic {
                line: token.line,
                column: token.col,
                message: "[warning] '!' must be followed by exactly one space".to_string(),
                rule: RULE,
            });
        }
    }
    diagnostics
}

/// Rewrites the whitespace immediately after every `!` negation operator so
/// it's exactly one space, closing the gap [`check`] flags (inserting a
/// space where there was none, and collapsing a longer run of spaces/tabs
/// down to one). A `!` on a line protected by a CreationKit fragment-code
/// wrapper (see [`fragment_code`]), or with nothing but a line ending/end of
/// file after it, is left exactly as-is — see [`check`].
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
        if token.kind != TokenKind::Not || protected[token.line] {
            continue;
        }
        let offset = line_starts[token.line - 1] + token.col - 1;
        let (start, end) = whitespace_run(bytes, offset);
        if starts_another_negation(bytes, start, end) {
            continue;
        }
        if !at_end_of_line(bytes, end) && !is_single_space(bytes, start, end) {
            ranges.push((start, end));
        }
    }

    if ranges.is_empty() {
        return source.to_string();
    }

    let mut repaired = String::with_capacity(source.len() + ranges.len());
    let mut previous = 0;
    for (start, end) in ranges {
        repaired.push_str(&source[previous..start]);
        repaired.push(' ');
        previous = end;
    }
    repaired.push_str(&source[previous..]);
    repaired
}

/// The `[start, end)` byte range of the run of spaces/tabs immediately
/// following the `!` at `offset`.
fn whitespace_run(bytes: &[u8], offset: usize) -> (usize, usize) {
    let start = offset + 1;
    let mut end = start;
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    (start, end)
}

fn is_single_space(bytes: &[u8], start: usize, end: usize) -> bool {
    end - start == 1 && bytes.get(start) == Some(&b' ')
}

/// Whether the `!` whose whitespace run is `[start, end)` is immediately
/// followed by another `!` with no space in between, i.e. `start == end`
/// (no whitespace at all was found) and the very next byte starts another
/// negation.
fn starts_another_negation(bytes: &[u8], start: usize, end: usize) -> bool {
    start == end && bytes.get(end) == Some(&b'!')
}

/// Whether `end` sits at the end of the line (a `\n`/`\r`) or end of file,
/// meaning the whitespace run examined by [`whitespace_run`] found nothing
/// but a line ending after it.
fn at_end_of_line(bytes: &[u8], end: usize) -> bool {
    !matches!(bytes.get(end), Some(byte) if *byte != b'\n' && *byte != b'\r')
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
#[path = "exclamation_spacing_tests.rs"]
mod tests;
