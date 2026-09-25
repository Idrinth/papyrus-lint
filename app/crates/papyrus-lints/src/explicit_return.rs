//! Flags a typed function/event whose control flow can reach the end of its
//! body without hitting a `Return` statement, since Papyrus then silently
//! returns that type's default value (`0`, `""`, `False`, or `None`)
//! instead of a value the author actually chose.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! the block structure of the function body; a script that doesn't parse
//! cleanly is left unchecked rather than guessed at. A function with no
//! declared return type isn't checked (falling off its end is the normal,
//! intended way for it to finish). A `While` loop is never assumed to
//! guarantee a `Return`, since it may run zero times; an `If` only
//! guarantees one when every branch (`If`/`ElseIf`, and an `Else`) does, so
//! an `If` with no `Else` never counts, matching the fact that its
//! condition might not match any branch at runtime. A native function
//! (no body to inspect) is never flagged.

use papyrus_parser::ast::{FunctionDecl, Stmt};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "explicit-return";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        if function.return_type.is_none() || function.is_native {
            return;
        }
        if body_always_returns(&function.body) {
            return;
        }
        self.store.emit(
            ctx.line,
            1,
            format!(
                "[error] Function '{}' does not return a value on every code path",
                function.name
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for typed functions/events with a code path that falls
/// off the end of the body without an explicit `Return`.
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

/// Whether every path through `body` is guaranteed to execute a `Return`
/// before falling off the end of the block.
fn body_always_returns(body: &[Stmt]) -> bool {
    body.iter().any(stmt_always_returns)
}

fn stmt_always_returns(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return { .. } => true,
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            body_always_returns(else_body)
                && branches
                    .iter()
                    .all(|branch| body_always_returns(&branch.body))
        }
        Stmt::While { .. } | Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } => false,
        Stmt::LockGuard {
            kind, body, else_body, ..
        } => match kind {
            papyrus_parser::ast::LockKind::Lock => body_always_returns(body),
            papyrus_parser::ast::LockKind::Try => {
                body_always_returns(body) && body_always_returns(else_body)
            }
        },
    }
}

#[cfg(test)]
#[path = "explicit_return_tests.rs"]
mod tests;
