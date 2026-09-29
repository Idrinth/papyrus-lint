//! Flags C-like escape sequences inside string literals.
//!
//! Papyrus string literals have no escape sequences: a literal `"\n"` is the
//! two characters backslash and `n`, not a newline. Authors coming from
//! C/JavaScript/Python constantly write `Debug.Trace("Loaded\nDone")` or
//! `"\t"` expecting whitespace escapes, then wonder why the log shows a
//! literal `\n`.
//!
//! This works on lexer tokens rather than the parsed AST so a script that
//! doesn't parse cleanly is still checked. The lexer itself interprets a
//! few backslash sequences when building the token value, so this lint
//! re-scans the raw source text between the quotes and only flags the
//! high-signal set `\n` / `\t` / `\r` — intentional Windows-style paths
//! (`"C:\Games"`) and doubled backslashes are left alone.

use papyrus_parser::token::{Token, TokenKind};

use crate::token_walk::line_starts;
use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "invalid-string-escape";

/// Escape letters this lint treats as common C-like misconceptions.
const FLAGGED_ESCAPES: &[u8] = b"ntr";

#[derive(Default)]
struct Collect {
    store: Store,
    line_starts: Vec<usize>,
}

impl TokenLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.line_starts = line_starts(ctx.source);
    }

    fn visit_token(
        &mut self,
        token: &Token,
        _index: usize,
        _tokens: &[Token],
        ctx: &mut VisitCtx<'_>,
    ) {
        if !matches!(token.kind, TokenKind::StringLiteral(_)) {
            return;
        }
        let Some(line_start) = token
            .line
            .checked_sub(1)
            .and_then(|line| self.line_starts.get(line))
        else {
            return;
        };
        let Some(column) = token.col.checked_sub(1) else {
            return;
        };
        let Some(offset) = line_start.checked_add(column) else {
            return;
        };
        visit_escapes_in_string(ctx.source, token.col, offset, |column, letter| {
            self.store.emit(
                token.line,
                column,
                format!(
                    "[warning] string literal contains '\\{letter}', but Papyrus has no \
                     escape sequences — the two characters backslash and '{letter}' are \
                     stored literally; prefer separate Debug.Trace calls (or omit the \
                     escape) instead of embedding C-like escapes"
                ),
                RULE,
            );
        });
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Checks `source` for string literals that contain a C-like `\n`, `\t`, or
/// `\r` escape sequence.
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

/// Walks the raw source of the string literal that starts at `offset`
/// (the opening `"`), calling `on_escape` with the 1-indexed column of each
/// flagged backslash and the escape letter that follows it.
fn visit_escapes_in_string(
    source: &str,
    open_col: usize,
    offset: usize,
    mut on_escape: impl FnMut(usize, char),
) {
    let bytes = source.as_bytes();
    if bytes.get(offset) != Some(&b'"') {
        return;
    }
    let mut index = offset + 1;
    let mut col = open_col + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return,
            b'\n' | b'\r' => return,
            b'\\' => {
                let Some(&escaped) = bytes.get(index + 1) else {
                    return;
                };
                if FLAGGED_ESCAPES.contains(&escaped) {
                    on_escape(col, escaped as char);
                }
                // Advance past both the backslash and the following byte,
                // matching how the lexer consumes an escape pair.
                index += 2;
                col += 2;
            }
            _ => {
                index += 1;
                col += 1;
            }
        }
    }
}

#[cfg(test)]
#[path = "invalid_string_escape_tests.rs"]
mod tests;
