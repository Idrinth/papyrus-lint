//! Shared visitor and repair for binary-operator spacing rules.
//!
//! [`operator_spacing`](crate::operator_spacing) and
//! [`assignment_operator_spacing`](crate::assignment_operator_spacing)
//! differ only in which token kinds they treat as operators and which rule
//! id they emit. The same-line gap check, fragment-code exemption, and
//! one-space repair live here so those two rules cannot drift apart.

use std::marker::PhantomData;

use crate::token_walk::line_starts;
use crate::{fragment_code, Diagnostic};
use papyrus_parser::token::{Token, TokenKind};

/// A token-spacing rule that flags (and repairs) the same one-space
/// requirement around a chosen set of binary operators.
pub(crate) trait BinaryOperatorSpacing: 'static {
    /// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
    const RULE: &'static str;

    /// The source text of a token this lint cares about, or `None` for
    /// every other token kind.
    fn operator_text(kind: &TokenKind) -> Option<&'static str>;
}

struct Collect<L> {
    store: crate::visitor::Store,
    protected: Vec<bool>,
    line_starts: Vec<usize>,
    _lint: PhantomData<L>,
}

impl<L> Default for Collect<L> {
    fn default() -> Self {
        Self {
            store: crate::visitor::Store::default(),
            protected: Vec::new(),
            line_starts: Vec::new(),
            _lint: PhantomData,
        }
    }
}

impl<L: BinaryOperatorSpacing> crate::visitor::TokenLint for Collect<L> {
    fn store(&mut self) -> &mut crate::visitor::Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut crate::visitor::VisitCtx<'_>) {
        self.protected = fragment_code::protected_lines(ctx.source);
        self.line_starts = line_starts(ctx.source);
    }

    fn visit_token(
        &mut self,
        token: &Token,
        _index: usize,
        _tokens: &[Token],
        ctx: &mut crate::visitor::VisitCtx<'_>,
    ) {
        let Some(text) = L::operator_text(&token.kind) else {
            return;
        };
        if self.protected[token.line] {
            return;
        }
        let offset = self.line_starts[token.line - 1] + token.col - 1;
        let end = offset + text.len();
        let bytes = ctx.source.as_bytes();

        if let Some((start, len)) = leading_gap(bytes, offset) {
            if !(len == 1 && bytes[start] == b' ') {
                self.store.emit(
                    token.line,
                    token.col,
                    format!("[warning] '{text}' must be preceded by exactly one space"),
                    L::RULE,
                );
            }
        }
        if let Some((start, gend)) = trailing_gap(bytes, end) {
            if !(gend - start == 1 && bytes[start] == b' ') {
                self.store.emit(
                    token.line,
                    token.col,
                    format!("[warning] '{text}' must be followed by exactly one space"),
                    L::RULE,
                );
            }
        }
    }
}

pub(crate) fn visitor<L: BinaryOperatorSpacing>() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::Tokens(Box::new(Collect::<L>::default()))
}

pub(crate) fn check<L: BinaryOperatorSpacing>(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor::<L>(), source, ast, tokens, config, external)
}

/// Normalizes the whitespace on either side of every operator selected by
/// `L` to exactly one space, applying the same same-line rule (and
/// fragment-code exemption) as [`check`]. A gap that reaches a newline is
/// left exactly as-is, so a statement continued across physical lines
/// keeps its own line breaks.
pub(crate) fn repair<L: BinaryOperatorSpacing>(
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
        let Some(text) = L::operator_text(&token.kind) else {
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
