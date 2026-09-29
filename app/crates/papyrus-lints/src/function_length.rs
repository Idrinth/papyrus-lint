//! Flags functions/events whose body is longer than a configured line count.
//!
//! Two independent maxima apply to the lines between the `Function`/`Event`
//! header and the matching `EndFunction`/`EndEvent`:
//!
//! - [`crate::config::Config::function_length_max_code_lines`] counts only
//!   lines that still contain code after comments and blank space are
//!   stripped (`{ ... }` documentation comments, `;` line comments, and
//!   `;/ ... /;` block comments do not count).
//! - [`crate::config::Config::function_length_max_lines`] counts every
//!   physical line, comments and blanks included.
//!
//! Native stubs have no body to measure. CreationKit fragment-wrapper
//! handlers (the generated `Function` whose header sits on a
//! `;BEGIN FRAGMENT CODE` protected line) are left alone so generated
//! boilerplate is not treated as an author-written block.

use papyrus_parser::ast::FunctionDecl;
use papyrus_parser::token::{Keyword, Token, TokenKind};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "function-length";

#[derive(Default)]
struct Collect {
    store: Store,
    protected: Vec<bool>,
    line_kinds: Vec<LineKind>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.protected = fragment_code::protected_lines(ctx.source);
        self.line_kinds = classify_lines(ctx.source);
    }

    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        if function.is_native {
            return;
        }
        if self.protected.get(function.line).copied().unwrap_or(false) {
            return;
        }
        let Some(tokens) = ctx.tokens else {
            return;
        };
        let Some((first, last)) = body_line_range(tokens, function) else {
            return;
        };

        let mut physical = 0usize;
        let mut code = 0usize;
        for line in first..=last {
            physical += 1;
            if self.line_kinds.get(line).copied() == Some(LineKind::Code) {
                code += 1;
            }
        }

        let max_code = ctx.config.function_length_max_code_lines;
        let max_lines = ctx.config.function_length_max_lines;
        let over_code = code > max_code;
        let over_lines = physical > max_lines;
        if !over_code && !over_lines {
            return;
        }

        let kind = if function.is_event {
            "Event"
        } else {
            "Function"
        };
        let message = match (over_code, over_lines) {
            (true, true) => format!(
                "[warning] {kind} '{}' has {code} lines of code (maximum: {max_code}) and {physical} lines (maximum: {max_lines})",
                function.name
            ),
            (true, false) => format!(
                "[warning] {kind} '{}' has {code} lines of code (maximum: {max_code})",
                function.name
            ),
            (false, true) => format!(
                "[warning] {kind} '{}' has {physical} lines (maximum: {max_lines})",
                function.name
            ),
            (false, false) => return,
        };
        self.store.emit(ctx.line, 1, message, RULE);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for functions/events whose body exceeds
/// `function_length_max_code_lines` or `function_length_max_lines`.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    Blank,
    Comment,
    Code,
}

fn classify_lines(source: &str) -> Vec<LineKind> {
    let line_count = source.lines().count() + 1;
    let mut kinds = vec![LineKind::Blank; line_count];
    let mut in_block = false;
    let mut in_brace = false;

    for (index, line) in source.lines().enumerate() {
        kinds[index + 1] = classify_line(line, &mut in_block, &mut in_brace);
    }
    kinds
}

fn classify_line(line: &str, in_block: &mut bool, in_brace: &mut bool) -> LineKind {
    let bytes = line.as_bytes();
    let mut pos = 0;
    let mut saw_code = false;
    let mut saw_comment = *in_block || *in_brace;

    while pos < bytes.len() {
        if *in_block {
            saw_comment = true;
            match line[pos..].find("/;") {
                Some(offset) => {
                    *in_block = false;
                    pos += offset + 2;
                }
                None => break,
            }
            continue;
        }
        if *in_brace {
            saw_comment = true;
            match line[pos..].find('}') {
                Some(offset) => {
                    *in_brace = false;
                    pos += offset + 1;
                }
                None => break,
            }
            continue;
        }

        match bytes[pos] {
            b' ' | b'\t' | b'\r' => pos += 1,
            b'"' => {
                saw_code = true;
                pos += 1;
                while pos < bytes.len() && bytes[pos] != b'"' {
                    pos += if bytes[pos] == b'\\' { 2 } else { 1 };
                }
                pos += 1;
            }
            b'{' => {
                saw_comment = true;
                *in_brace = true;
                pos += 1;
            }
            b';' if bytes.get(pos + 1) == Some(&b'/') => {
                saw_comment = true;
                *in_block = true;
                pos += 2;
            }
            b';' => {
                saw_comment = true;
                break;
            }
            _ => {
                saw_code = true;
                pos += 1;
            }
        }
    }

    if saw_code {
        LineKind::Code
    } else if saw_comment {
        LineKind::Comment
    } else {
        LineKind::Blank
    }
}

fn body_line_range(tokens: &[Token], function: &FunctionDecl) -> Option<(usize, usize)> {
    let start_kind = if function.is_event {
        Keyword::Event
    } else {
        Keyword::Function
    };
    let end_kind = if function.is_event {
        Keyword::EndEvent
    } else {
        Keyword::EndFunction
    };
    let start = tokens.iter().position(|token| {
        token.line == function.line && matches!(token.kind, TokenKind::Keyword(kind) if kind == start_kind)
    })?;
    let header_end_line = tokens[start..]
        .iter()
        .find(|token| matches!(token.kind, TokenKind::Newline))
        .map_or(function.line, |token| token.line);
    let end_line = tokens[start + 1..]
        .iter()
        .find(|token| matches!(token.kind, TokenKind::Keyword(kind) if kind == end_kind))
        .map(|token| token.line)?;
    let first = header_end_line + 1;
    let last = end_line.saturating_sub(1);
    if first > last {
        None
    } else {
        Some((first, last))
    }
}

#[cfg(test)]
#[path = "function_length_tests.rs"]
mod tests;
