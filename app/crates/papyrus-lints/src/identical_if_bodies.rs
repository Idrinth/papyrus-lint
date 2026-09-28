//! Flags consecutive `If` / `ElseIf` branches whose bodies are the same,
//! since that is often a copy-paste oversight (a forgotten condition or
//! body change) rather than something intentional.
//!
//! Only immediately adjacent branches of the same `If` are compared — the
//! `If` against its first `ElseIf`, or one `ElseIf` against the next. An
//! `Else` body is never compared this way: it is not an `If`/`ElseIf`
//! branch, and an identical fallback can be deliberate. Empty bodies are
//! left to [`crate::empty_body`] rather than double-flagged here.
//!
//! Bodies are compared structurally, ignoring source locations (line /
//! column) so two copy-pasted blocks on different lines still match. A
//! script that does not parse cleanly is left unchecked.

use papyrus_parser::ast::{Expr, IfBranch, Literal, Stmt, VariableDecl};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "identical-if-bodies";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        let Stmt::If { branches, .. } = stmt else {
            return;
        };
        check_consecutive_branches(branches, &mut self.store);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks consecutive `If` / `ElseIf` branches in `source` for identical
/// non-empty bodies. Flagged as an `[info]`, since this is a hint that
/// something may have been forgotten rather than a hard correctness error.
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

/// Flags each later branch whose non-empty body is structurally identical
/// to the immediately preceding branch's body.
fn check_consecutive_branches(branches: &[IfBranch], store: &mut Store) {
    for window in branches.windows(2) {
        let earlier = &window[0];
        let later = &window[1];
        if later.body.is_empty() || earlier.body.is_empty() {
            continue;
        }
        if bodies_equal(&earlier.body, &later.body) {
            store.emit(
                later.line,
                later.col,
                format!(
                    "[info] This If/ElseIf body is identical to the previous branch on line {}; \
                     this may be a copy-paste oversight",
                    earlier.line
                ),
                RULE,
            );
        }
    }
}

fn bodies_equal(left: &[Stmt], right: &[Stmt]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| stmts_equal(a, b))
}

/// Structural equality for statements, ignoring line / column positions so
/// two copy-pasted bodies on different lines still compare equal.
fn stmts_equal(left: &Stmt, right: &Stmt) -> bool {
    match (left, right) {
        (Stmt::VarDecl(a), Stmt::VarDecl(b)) => var_decls_equal(a, b),
        (
            Stmt::Assign {
                target: target_a,
                op: op_a,
                value: value_a,
                ..
            },
            Stmt::Assign {
                target: target_b,
                op: op_b,
                value: value_b,
                ..
            },
        ) => op_a == op_b && exprs_equal(target_a, target_b) && exprs_equal(value_a, value_b),
        (Stmt::Expr { value: a, .. }, Stmt::Expr { value: b, .. }) => exprs_equal(a, b),
        (Stmt::Return { value: a, .. }, Stmt::Return { value: b, .. }) => match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => exprs_equal(a, b),
            _ => false,
        },
        (
            Stmt::If {
                branches: branches_a,
                else_body: else_a,
                else_line: else_line_a,
                ..
            },
            Stmt::If {
                branches: branches_b,
                else_body: else_b,
                else_line: else_line_b,
                ..
            },
        ) => {
            // Presence of an Else clause (not its location) matters; both
            // "no Else" and "empty Else" leave else_body empty, so the
            // Option line is what distinguishes them.
            else_line_a.is_some() == else_line_b.is_some()
                && branches_a.len() == branches_b.len()
                && branches_a
                    .iter()
                    .zip(branches_b)
                    .all(|(a, b)| if_branches_equal(a, b))
                && bodies_equal(else_a, else_b)
        }
        (
            Stmt::While {
                condition: cond_a,
                body: body_a,
                ..
            },
            Stmt::While {
                condition: cond_b,
                body: body_b,
                ..
            },
        ) => exprs_equal(cond_a, cond_b) && bodies_equal(body_a, body_b),
        (
            Stmt::LockGuard {
                kind: kind_a,
                names: names_a,
                body: body_a,
                else_body: else_a,
                else_line: else_line_a,
                ..
            },
            Stmt::LockGuard {
                kind: kind_b,
                names: names_b,
                body: body_b,
                else_body: else_b,
                else_line: else_line_b,
                ..
            },
        ) => {
            kind_a == kind_b
                && names_a == names_b
                && else_line_a.is_some() == else_line_b.is_some()
                && bodies_equal(body_a, body_b)
                && bodies_equal(else_a, else_b)
        }
        _ => false,
    }
}

fn if_branches_equal(left: &IfBranch, right: &IfBranch) -> bool {
    exprs_equal(&left.condition, &right.condition) && bodies_equal(&left.body, &right.body)
}

fn var_decls_equal(left: &VariableDecl, right: &VariableDecl) -> bool {
    left.type_name == right.type_name
        && left.name == right.name
        && left.is_conditional == right.is_conditional
        && left.requires_guard == right.requires_guard
        && match (&left.value, &right.value) {
            (None, None) => true,
            (Some(a), Some(b)) => exprs_equal(a, b),
            _ => false,
        }
}

fn exprs_equal(left: &Expr, right: &Expr) -> bool {
    match (left, right) {
        (Expr::Literal(a), Expr::Literal(b)) => literals_equal(a, b),
        (Expr::Identifier(a), Expr::Identifier(b)) => a == b,
        (Expr::Self_, Expr::Self_) | (Expr::Parent, Expr::Parent) => true,
        (
            Expr::Binary {
                left: la,
                op: op_a,
                right: ra,
            },
            Expr::Binary {
                left: lb,
                op: op_b,
                right: rb,
            },
        ) => op_a == op_b && exprs_equal(la, lb) && exprs_equal(ra, rb),
        (
            Expr::Unary {
                op: op_a,
                operand: a,
            },
            Expr::Unary {
                op: op_b,
                operand: b,
            },
        ) => op_a == op_b && exprs_equal(a, b),
        (
            Expr::Call {
                callee: ca,
                args: aa,
                ..
            },
            Expr::Call {
                callee: cb,
                args: ab,
                ..
            },
        ) => {
            exprs_equal(ca, cb)
                && aa.len() == ab.len()
                && aa.iter().zip(ab).all(|(a, b)| exprs_equal(a, b))
        }
        (
            Expr::NamedArg {
                name: na,
                value: va,
            },
            Expr::NamedArg {
                name: nb,
                value: vb,
            },
        ) => na == nb && exprs_equal(va, vb),
        (
            Expr::Member {
                object: oa,
                property: pa,
            },
            Expr::Member {
                object: ob,
                property: pb,
            },
        ) => pa == pb && exprs_equal(oa, ob),
        (
            Expr::Index {
                object: oa,
                index: ia,
            },
            Expr::Index {
                object: ob,
                index: ib,
            },
        ) => exprs_equal(oa, ob) && exprs_equal(ia, ib),
        (
            Expr::Cast {
                value: va,
                type_name: ta,
            },
            Expr::Cast {
                value: vb,
                type_name: tb,
            },
        ) => ta == tb && exprs_equal(va, vb),
        (
            Expr::Is {
                value: va,
                type_name: ta,
            },
            Expr::Is {
                value: vb,
                type_name: tb,
            },
        ) => ta == tb && exprs_equal(va, vb),
        (
            Expr::NewArray {
                type_name: ta,
                size: sa,
            },
            Expr::NewArray {
                type_name: tb,
                size: sb,
            },
        ) => ta == tb && exprs_equal(sa, sb),
        (Expr::NewStruct { type_name: a }, Expr::NewStruct { type_name: b }) => a == b,
        _ => false,
    }
}

fn literals_equal(left: &Literal, right: &Literal) -> bool {
    match (left, right) {
        (
            Literal::Int {
                value: a,
                format: fa,
            },
            Literal::Int {
                value: b,
                format: fb,
            },
        ) => a == b && fa == fb,
        (Literal::Float(a), Literal::Float(b)) => a.to_bits() == b.to_bits(),
        (Literal::String(a), Literal::String(b)) => a == b,
        (Literal::Bool(a), Literal::Bool(b)) => a == b,
        (Literal::None, Literal::None) => true,
        _ => false,
    }
}

#[cfg(test)]
#[path = "identical_if_bodies_tests.rs"]
mod tests;
