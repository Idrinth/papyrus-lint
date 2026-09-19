//! Flags `If`/`ElseIf`/`While` conditions that are compile-time constants
//! (e.g. `If true`, `If 1 == 2`), so they always take (or always skip) the
//! branch/loop they guard regardless of runtime state.
//!
//! Like the other AST-based lints in this crate, this one only looks at
//! expressions built entirely from literals (optionally combined with
//! arithmetic, comparison, logical, and unary operators); an expression
//! that references an identifier, a call, `Self`/`Parent`, a member/index
//! access, a cast, or a `new` array is left unflagged rather than guessed
//! at, since its value can't be known without running the script.

use papyrus_parser::ast::{Expr, IfBranch, Stmt};

use crate::const_eval::{eval_const, truthy};
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "static-condition";

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

/// Checks every `If`/`ElseIf`/`While` condition in `source` and flags the
/// ones that evaluate to a constant `true` or `false` regardless of
/// runtime state.
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

fn check_condition(condition: &Expr, line: usize, column: usize, store: &mut Store) {
    let Some(value) = eval_const(condition) else {
        return;
    };

    let always = if truthy(&value) { "true" } else { "false" };
    store.emit(
        line,
        column,
        format!(
            "[warning] Condition is always {always}; it does not depend on any runtime value"
        ),
        RULE,
    );
}

#[cfg(test)]
#[path = "static_condition_tests.rs"]
mod tests;
