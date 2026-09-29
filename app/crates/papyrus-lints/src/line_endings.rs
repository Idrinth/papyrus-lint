//! Flags line terminators that do not match `line_endings_mode`.

use crate::config::LineEndingsMode;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` comments.
pub const RULE: &str = "line-endings";

/// Checks that every terminator in `source` matches `config.line_endings_mode`.
/// A file with no terminators is left to `final-newline`.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (ast, tokens, external);

    if let Some((line, column)) = first_offending_terminator(source, config.line_endings_mode) {
        return vec![Diagnostic {
            line,
            column,
            message: format!(
                "[warning] Line ending is not {}",
                config.line_endings_mode.label()
            ),
            rule: RULE,
        }];
    }
    Vec::new()
}

/// Rewrites every terminator to the configured mode. Does not invent a missing
/// final newline.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens);
    rewrite(source, config.line_endings_mode)
}

fn first_offending_terminator(source: &str, mode: LineEndingsMode) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut line = 1usize;
    let mut column = 1usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            if mode != LineEndingsMode::Crlf {
                return Some((line, column));
            }
            line += 1;
            column = 1;
            i += 2;
            continue;
        }
        if bytes[i] == b'\n' || bytes[i] == b'\r' {
            if mode != LineEndingsMode::Lf {
                return Some((line, column));
            }
            line += 1;
            column = 1;
            i += 1;
            continue;
        }
        column += 1;
        i += 1;
    }
    None
}

fn rewrite(source: &str, mode: LineEndingsMode) -> String {
    let replacement = match mode {
        LineEndingsMode::Lf => "\n",
        LineEndingsMode::Crlf => "\r\n",
    };
    let mut out = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            out.push_str(replacement);
            i += 2;
            continue;
        }
        if bytes[i] == b'\n' || bytes[i] == b'\r' {
            out.push_str(replacement);
            i += 1;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

#[cfg(test)]
#[path = "line_endings_tests.rs"]
mod tests;
