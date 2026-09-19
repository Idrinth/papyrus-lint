//! Flags a plain `=` assignment whose right-hand side is the exact same
//! reference as its own target (e.g. `a = a`, `Self.Foo = Self.Foo`,
//! `akRef.Foo = akRef.Foo`), since that assignment can never change the
//! value it reads and is almost always a copy-paste mistake or leftover
//! from a refactor.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to compare the target and value expressions structurally; a script
//! that doesn't parse cleanly is left unchecked rather than guessed at.
//!
//! Only a bare identifier or a chain of member accesses rooted at one (or
//! at `Self`) is ever compared this way — a call, an index, or any other
//! expression shape never counts as a self-assignment, since re-evaluating
//! it on both sides of the same line isn't guaranteed to read the same
//! value twice (or may have side effects of its own). A compound
//! assignment (`+=`, `-=`, ...) is never flagged either, since unlike
//! plain `=` it does change the target's value.

use papyrus_parser::ast::{AssignOp, Expr, Stmt};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "self-assignment";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_stmt(&mut self, stmt: &Stmt, ctx: &mut VisitCtx<'_>) {
        let Stmt::Assign {
            target,
            op: AssignOp::Assign,
            value,
            ..
        } = stmt
        else {
            return;
        };
        let (Some(target_key), Some(value_key)) = (reference_key(target), reference_key(value))
        else {
            return;
        };
        if target_key != value_key {
            return;
        }
        self.store.emit(
            ctx.line,
            1,
            "[warning] This assigns a value to itself, which has no effect",
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for a plain `=` assignment whose target and value are the
/// exact same simple reference. Flagged as a `[warning]`.
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

/// A canonical, case-insensitive key identifying a "simple reference"
/// expression (a bare identifier, `Self`, or a chain of member accesses
/// rooted at either), or `None` for any other expression shape (a call, an
/// index, a literal, ...), which is never compared for self-assignment.
fn reference_key(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(name) => Some(name.to_ascii_lowercase()),
        Expr::Self_ => Some("self".to_string()),
        Expr::Member { object, property } => Some(format!(
            "{}.{}",
            reference_key(object)?,
            property.to_ascii_lowercase()
        )),
        _ => None,
    }
}

#[cfg(test)]
#[path = "self_assignment_tests.rs"]
mod tests;
