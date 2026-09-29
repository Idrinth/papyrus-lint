//! Flags a clause in a compound `&&` / `||` condition that is dominated or
//! duplicated by another clause on the same identifier.
//!
//! Conservative: same local/property identifier + constant numeric thresholds;
//! no guessing across calls or aliases.

use papyrus_parser::ast::{BinaryOp, Expr, IfBranch, Literal, Stmt, UnaryOp};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "redundant-condition";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_if_branch(&mut self, branch: &IfBranch, _ctx: &mut VisitCtx<'_>) {
        check_condition(&branch.condition, branch.line, branch.col, &mut self.store);
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        let Stmt::While {
            condition,
            line,
            col,
            ..
        } = stmt
        else {
            return;
        };
        check_condition(condition, *line, *col, &mut self.store);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every `If`/`ElseIf`/`While` condition in `source` for a redundant
/// clause under `&&` / `||`.
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

include!("redundant_condition_logic.rs");

#[cfg(test)]
#[path = "redundant_condition_tests.rs"]
mod tests;
