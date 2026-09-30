//! Flags getter calls used as standalone statements.
//!
//! A discarded `Get*` call is an expression-statement question. Scripts
//! that do not parse are left unchecked.

use papyrus_parser::ast::{Expr, Stmt};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-getter";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        if let Stmt::Expr { value, .. } = stmt {
            flag_discarded(value, &mut self.store);
        }
    }
}

/// Report at most one getter per discarded expression, matching the old
/// token visitor's `find_map` over top-level operands.
fn flag_discarded(expr: &Expr, store: &mut Store) -> bool {
    match expr {
        Expr::Call {
            callee,
            line,
            col,
            ..
        } => {
            if let Some(name) = call_name(callee) {
                if name
                    .get(..3)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("get"))
                {
                    let column = col.saturating_sub(name.len());
                    store.emit(
                        *line,
                        column.max(1),
                        format!(
                            "[warning] Getter '{name}' is called without using its return value"
                        ),
                        RULE,
                    );
                    return true;
                }
            }
            false
        }
        Expr::Binary { left, right, .. } => {
            flag_discarded(left, store) || flag_discarded(right, store)
        }
        Expr::Unary { operand, .. } => flag_discarded(operand, store),
        _ => false,
    }
}

fn call_name(callee: &Expr) -> Option<&str> {
    match callee {
        Expr::Identifier(name) => Some(name),
        Expr::Member { property, .. } => Some(property),
        _ => None,
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks for calls whose function name begins with `Get` and whose result is
/// discarded rather than assigned, returned, or used by another expression.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    if ast.is_none() {
        let wrapped = format!("ScriptName _UnusedGetterSnippet\n{source}");
        if let Ok(wrapped_ast) = papyrus_parser::parse(&wrapped) {
            let mut diagnostics =
                crate::visitor::run(visitor(), &wrapped, Some(&wrapped_ast), tokens, config, external);
            for diagnostic in &mut diagnostics {
                diagnostic.line = diagnostic.line.saturating_sub(1);
            }
            return diagnostics;
        }
    }
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

#[cfg(test)]
#[path = "unused_getter_tests.rs"]
mod tests;
