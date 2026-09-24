//! Flags unsafe uses of `ObjectReference.PlaceAtMe` and
//! `ObjectReference.PlaceActorAtMe`: discarding either function's returned
//! reference makes the newly created object difficult to clean up later, and
//! asking `PlaceAtMe` to create multiple objects returns only one reference.
//!
//! Receiver types cannot always be resolved locally, so, like several other
//! method-specific rules, this matches the method names case-insensitively on
//! any explicitly qualified call. A non-constant `PlaceAtMe` count is left
//! unflagged rather than guessed at.

use papyrus_parser::ast::{Expr, Stmt};

use crate::const_eval::eval_const_int;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "place-at-me";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        let Stmt::Expr { value, line } = stmt else {
            return;
        };
        let Expr::Call { callee, .. } = value else {
            return;
        };
        let Some(function) = matching_function(callee) else {
            return;
        };
        self.store.emit(
            *line,
            1,
            format!(
                "[warning] The reference returned by {function}(...) is not stored in a variable; store it so the created object can be cleaned up later"
            ),
            RULE,
        );
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Expr::Call { callee, args, .. } = expr else {
            return;
        };
        if matching_function(callee) != Some("PlaceAtMe") {
            return;
        }
        let Some(count) = args.get(1).and_then(argument_value).and_then(eval_const_int) else {
            return;
        };
        if count > 1 {
            self.store.emit(
                ctx.line,
                1,
                format!(
                    "[warning] PlaceAtMe creates {count} objects at once but returns only one reference; spawn one object per call so every created object can be retrieved and cleaned up"
                ),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `PlaceAtMe`/`PlaceActorAtMe` calls for discarded return values and
/// checks `PlaceAtMe` for a constant count greater than one.
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

fn matching_function(callee: &Expr) -> Option<&'static str> {
    let Expr::Member { property, .. } = callee else {
        return None;
    };
    ["PlaceAtMe", "PlaceActorAtMe"]
        .into_iter()
        .find(|name| name.eq_ignore_ascii_case(property))
}

fn argument_value(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::NamedArg { value, .. } => Some(value),
        other => Some(other),
    }
}

#[cfg(test)]
#[path = "place_at_me_tests.rs"]
mod tests;
