//! Requires whitespace after commas in parenthesized argument lists.

use crate::{fragment_code, Diagnostic};
use papyrus_parser::token::{Token, TokenKind};
use crate::token_walk::{line_starts};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "comma-spacing";

#[derive(Default)]
struct Collect {
    store: crate::visitor::Store,
    paren_depth: usize,
    protected: Vec<bool>,
    line_starts: Vec<usize>,
}

impl crate::visitor::TokenLint for Collect {
    fn store(&mut self) -> &mut crate::visitor::Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut crate::visitor::VisitCtx<'_>) {
        self.protected = fragment_code::protected_lines(ctx.source);
        self.line_starts = line_starts(ctx.source);
        self.paren_depth = 0;
    }

    fn visit_token(
        &mut self,
        token: &Token,
        _index: usize,
        _tokens: &[Token],
        ctx: &mut crate::visitor::VisitCtx<'_>,
    ) {
        match token.kind {
            TokenKind::LParen => self.paren_depth += 1,
            TokenKind::RParen => self.paren_depth = self.paren_depth.saturating_sub(1),
            TokenKind::Comma if self.paren_depth > 0 => {
                if self.protected.get(token.line).copied().unwrap_or(false) {
                    return;
                }
                let line_start = self.line_starts[token.line - 1];
                let offset = line_start + token.col - 1;
                let next = ctx.source.as_bytes().get(offset + 1).copied();
                if next.is_some_and(|byte| !byte.is_ascii_whitespace() && byte != b')') {
                    let column = ctx.source[line_start..offset].chars().count() + 1;
                    self.store.emit(
                        token.line,
                        column,
                        "[warning] Comma in argument list must be followed by whitespace",
                        RULE,
                    );
                }
            }
            _ => {}
        }
    }
}

pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Checks for argument-list commas that are immediately followed by another
/// non-whitespace character. Commas on a line protected by a CreationKit
/// fragment-code wrapper (see [`fragment_code`]) are never flagged.
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

/// Inserts one space after every unspaced comma in an argument list. Commas
/// on a line protected by a CreationKit fragment-code wrapper (see
/// [`fragment_code`]) are left exactly as-is.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);

    let protected = fragment_code::protected_lines(source);
    let offsets: Vec<_> = comma_offsets(source)
        .into_iter()
        .filter(|(_, line, _)| !protected[*line])
        .map(|(offset, _, _)| offset)
        .collect();
    if offsets.is_empty() {
        return source.to_string();
    }

    let mut repaired = String::with_capacity(source.len() + offsets.len());
    let mut previous = 0;
    for offset in offsets {
        let after_comma = offset + 1;
        repaired.push_str(&source[previous..after_comma]);
        repaired.push(' ');
        previous = after_comma;
    }
    repaired.push_str(&source[previous..]);
    repaired
}

fn comma_offsets(source: &str) -> Vec<(usize, usize, usize)> {
    match papyrus_parser::tokenize(source) {
        Ok(tokens) => comma_offsets_from_tokens(source, &tokens),
        Err(_) => Vec::new(),
    }
}

fn comma_offsets_from_tokens(source: &str, tokens: &[Token]) -> Vec<(usize, usize, usize)> {
    let line_starts = line_starts(source);
    let mut paren_depth = 0usize;
    let mut commas = Vec::new();

    for token in tokens {
        match token.kind {
            TokenKind::LParen => paren_depth += 1,
            TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::Comma if paren_depth > 0 => {
                let line_start = line_starts[token.line - 1];
                let offset = line_start + token.col - 1;
                let next = source.as_bytes().get(offset + 1).copied();
                if next.is_some_and(|byte| !byte.is_ascii_whitespace() && byte != b')') {
                    let column = source[line_start..offset].chars().count() + 1;
                    commas.push((offset, token.line, column));
                }
            }
            _ => {}
        }
    }
    commas
}

#[cfg(test)]
#[path = "comma_spacing_tests.rs"]
mod tests;
