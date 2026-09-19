//! Flags statements that appear after a `Return` within the same block,
//! since control flow can never reach them.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! the block structure of the function body; a script that doesn't parse
//! cleanly is left unchecked rather than guessed at.

use papyrus_parser::ast::{FunctionDecl, Stmt};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unreachable-statement";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        check_body(&function.body, &mut self.store);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for statements that follow a `Return` in the same
/// block (a function/event body, an `If`/`ElseIf`/`Else` branch, or a
/// `While` body). Flagged as a `[warning]`.
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

fn check_body(body: &[Stmt], store: &mut Store) {
    let mut returned = false;
    for stmt in body {
        if returned {
            store.emit(
                stmt_line(stmt),
                1,
                "[warning] Unreachable statement: this can never execute because the \
                          block already returned above it",
                RULE,
            );
        }
        match stmt {
            Stmt::Return { .. } => returned = true,
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    check_body(&branch.body, store);
                }
                check_body(else_body, store);
            }
            Stmt::While { body, .. } => check_body(body, store),
            _ => {}
        }
    }
}

fn stmt_line(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::VarDecl(decl) => decl.line,
        Stmt::Assign { line, .. } => *line,
        Stmt::Expr { line, .. } => *line,
        Stmt::Return { line, .. } => *line,
        Stmt::If { line, .. } => *line,
        Stmt::While { line, .. } => *line,
    }
}

#[cfg(test)]
#[path = "unreachable_statement_tests.rs"]
mod tests;
