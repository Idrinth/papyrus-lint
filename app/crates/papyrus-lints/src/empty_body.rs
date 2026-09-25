//! Flags `While` loops, `If`/`ElseIf` branches, and `Else` blocks whose body
//! has no real effect, since that's almost always a forgotten piece of
//! logic rather than something intentional.
//!
//! A `While` loop counts as empty both when it has no statements at all and
//! when every statement in it only nudges a variable by a constant amount
//! (`i += 1`, `i -= 1`, or the equivalent `i = i + 1`/`i = i - 1`) — the
//! common "loop until a counter reaches some value" idiom, but with nothing
//! else in the loop that would give it a purpose. A step built from
//! anything but a literal (a call, another variable, ...) has a side effect
//! of its own and isn't considered trivial.
//!
//! `If`/`ElseIf` branches are checked straight from the parsed AST, since
//! an empty branch body is unambiguous there. An `Else` block is too: the
//! AST's `Stmt::If` records the line/column the `Else` keyword itself
//! started on (`None` when there was no `Else` clause at all), which is
//! what lets "no `Else` clause" be told apart from "an empty `Else`
//! clause" — both leave `else_body` empty — without re-lexing the source.
//!
//! A script that doesn't parse cleanly is simply left unchecked by the
//! `While`/`If`/`ElseIf`/`Else` checks above, all of which run from the
//! AST; as a fallback for that case only, the `Else` check also scans the
//! token stream directly for an `Else` keyword immediately followed (only
//! whitespace/newlines between them) by `EndIf`, so it still runs on a
//! script that doesn't parse.

use papyrus_parser::ast::{AssignOp, BinaryOp, Expr, IfBranch, Literal, Stmt};
use papyrus_parser::token::{Keyword, Token, TokenKind};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "empty-body";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_if_branch(&mut self, branch: &IfBranch, _ctx: &mut VisitCtx<'_>) {
        if branch.body.is_empty() {
            self.store.emit(
                branch.line,
                branch.col,
                "[warning] Empty If/ElseIf body; this looks like an oversight rather than \
                 something intentional",
                RULE,
            );
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        match stmt {
            Stmt::If {
                else_body,
                else_line,
                else_col,
                ..
            } => {
                if let (Some(line), Some(column)) = (else_line, else_col) {
                    if else_body.is_empty() {
                        self.store.emit(
                            *line,
                            *column,
                            "[warning] Empty Else body; this looks like an oversight \
                             rather than something intentional",
                            RULE,
                        );
                    }
                }
            }
            Stmt::While {
                body, line, col, ..
            } => {
                if body.is_empty() {
                    self.store.emit(
                        *line,
                        *col,
                        "[warning] Loop body is empty; this looks like an oversight \
                         rather than something intentional",
                        RULE,
                    );
                } else if is_trivial_loop_body(body) {
                    self.store.emit(
                        *line,
                        *col,
                        "[warning] Loop only increments or decrements a variable, \
                         with no other effect; this looks like an oversight rather \
                         than something intentional",
                        RULE,
                    );
                }
            }
            Stmt::LockGuard {
                body,
                else_body,
                else_line,
                else_col,
                line,
                col,
                ..
            } => {
                if body.is_empty() {
                    self.store.emit(
                        *line,
                        *col,
                        "[warning] LockGuard body is empty; this looks like an oversight \
                         rather than something intentional",
                        RULE,
                    );
                }
                if let (Some(line), Some(column)) = (else_line, else_col) {
                    if else_body.is_empty() {
                        self.store.emit(
                            *line,
                            *column,
                            "[warning] Empty ElseTryLockGuard body; this looks like an oversight \
                             rather than something intentional",
                            RULE,
                        );
                    }
                }
            }
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }

    fn finish(&mut self, ctx: &mut VisitCtx<'_>) {
        if ctx.ast.is_none() {
            self.store.extend(empty_else_diagnostics(ctx.tokens));
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for `While` loops with no real effect and empty
/// `If`/`ElseIf`/`Else` bodies. Flagged as a `[warning]`, since this is
/// almost always an oversight rather than something intentional.
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

/// Whether every statement in a (non-empty) loop body is nothing more than
/// a constant step applied to some variable.
fn is_trivial_loop_body(body: &[Stmt]) -> bool {
    body.iter().all(|stmt| match stmt {
        Stmt::Assign {
            target, op, value, ..
        } => is_constant_step(target, *op, value),
        _ => false,
    })
}

/// Whether `target <op> value` amounts to nudging `target` by a constant,
/// e.g. `i += 1`, `i -= 1`, `i = i + 1`, or `i = i - 1`. Anything the step
/// depends on beyond a literal (a call, another variable, ...) has a side
/// effect of its own and isn't considered trivial.
fn is_constant_step(target: &Expr, op: AssignOp, value: &Expr) -> bool {
    let Expr::Identifier(name) = target else {
        return false;
    };

    match op {
        AssignOp::AddAssign | AssignOp::SubAssign => is_numeric_literal(value),
        AssignOp::Assign => match value {
            Expr::Binary {
                left,
                op: BinaryOp::Add,
                right,
            } => {
                (is_identifier(left, name) && is_numeric_literal(right))
                    || (is_numeric_literal(left) && is_identifier(right, name))
            }
            Expr::Binary {
                left,
                op: BinaryOp::Sub,
                right,
            } => is_identifier(left, name) && is_numeric_literal(right),
            _ => false,
        },
        AssignOp::MulAssign | AssignOp::DivAssign | AssignOp::ModAssign => false,
    }
}

fn is_identifier(expr: &Expr, name: &str) -> bool {
    matches!(expr, Expr::Identifier(other) if other.eq_ignore_ascii_case(name))
}

fn is_numeric_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Literal(Literal::Int { .. }) | Expr::Literal(Literal::Float(_))
    )
}

/// Fallback for a script that doesn't parse cleanly (see [`check`]): scans
/// `source`'s lexer tokens directly for an `Else` keyword immediately
/// followed (modulo newlines) by `EndIf`.
fn empty_else_diagnostics(tokens: Option<&[Token]>) -> Vec<Diagnostic> {
    let Some(tokens) = tokens else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Keyword(Keyword::Else) {
            continue;
        }
        let next = tokens[index + 1..]
            .iter()
            .find(|token| token.kind != TokenKind::Newline);
        if matches!(next, Some(token) if token.kind == TokenKind::Keyword(Keyword::EndIf)) {
            diagnostics.push(Diagnostic {
                line: token.line,
                column: token.col,
                message: "[warning] Empty Else body; this looks like an oversight rather than \
                          something intentional"
                    .to_string(),
                rule: RULE,
            });
        }
    }
    diagnostics
}

#[cfg(test)]
#[path = "empty_body_tests.rs"]
mod tests;
