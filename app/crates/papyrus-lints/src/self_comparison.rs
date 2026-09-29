//! Flags a comparison (`==`, `!=`, `<=`, or `>=`) whose left and right
//! sides are the exact same simple reference (e.g. `count == count`,
//! `actor != actor`, `Self.Foo <= Self.Foo`), since that comparison is
//! always true (`==`/`<=`/`>=`) or always false (`!=`) and is almost
//! always a typo or unfinished edit.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to compare the operands structurally; a script that doesn't parse
//! cleanly is left unchecked rather than guessed at.
//!
//! Only a bare identifier or a chain of member accesses rooted at one
//! (or at `Self`/`Parent`) is ever compared this way — a call, an index,
//! or any other expression shape never counts as a self-comparison, since
//! re-evaluating it on both sides isn't guaranteed to read the same value
//! twice (or may have side effects of its own). Float NaN quirks are
//! irrelevant in Papyrus, so the rule stays simple.

use papyrus_parser::ast::{BinaryOp, Expr};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "self-comparison";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Expr::Binary { left, op, right } = expr else {
            return;
        };
        let Some(outcome) = self_comparison_outcome(*op) else {
            return;
        };
        let (Some(left_key), Some(right_key)) = (reference_key(left), reference_key(right)) else {
            return;
        };
        if left_key != right_key {
            return;
        }
        self.store.emit(
            ctx.line,
            1,
            format!("[warning] This compares a value to itself, which is always {outcome}"),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for a comparison whose operands are the exact same
/// simple reference. Flagged as a `[warning]`.
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

/// Returns `"true"` or `"false"` for operators that are constant when both
/// operands are the same simple reference; `None` for every other op.
fn self_comparison_outcome(op: BinaryOp) -> Option<&'static str> {
    match op {
        BinaryOp::Eq | BinaryOp::LtEq | BinaryOp::GtEq => Some("true"),
        BinaryOp::NotEq => Some("false"),
        _ => None,
    }
}

/// A canonical, case-insensitive key identifying a "simple reference"
/// expression (a bare identifier, `Self`, `Parent`, or a chain of member
/// accesses rooted at either), or `None` for any other expression shape
/// (a call, an index, a literal, ...), which is never compared for
/// self-comparison.
fn reference_key(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(name) => Some(name.to_ascii_lowercase()),
        Expr::Self_ => Some("self".to_string()),
        Expr::Parent => Some("parent".to_string()),
        Expr::Member { object, property } => Some(format!(
            "{}.{}",
            reference_key(object)?,
            property.to_ascii_lowercase()
        )),
        _ => None,
    }
}

#[cfg(test)]
#[path = "self_comparison_tests.rs"]
mod tests;
