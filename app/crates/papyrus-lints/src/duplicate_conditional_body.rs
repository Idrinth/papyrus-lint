//! Flags adjacent `If` / `ElseIf` / `Else` branches whose bodies are the same,
//! since that is usually a forgotten edit (the condition was copied, the
//! body was left unchanged) rather than something intentional.
//!
//! Only neighboring arms of the same chain are compared — `If` vs the
//! first `ElseIf`, one `ElseIf` vs the next, or the last `If`/`ElseIf`
//! vs `Else`. A later arm that repeats an earlier body with a different
//! body in between is left alone. Empty bodies are left to `empty-body`.
//!
//! Bodies are compared structurally (statement and expression shape,
//! identifiers case-insensitively) so line numbers, columns, and
//! whitespace alone do not trigger.

use papyrus_parser::ast::{Expr, Literal, Stmt, TypeName, VariableDecl};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "duplicate-conditional-body";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        let Stmt::If {
            branches,
            else_body,
            else_line,
            else_col,
            ..
        } = stmt
        else {
            return;
        };
        for window in branches.windows(2) {
            let previous = &window[0];
            let current = &window[1];
            if previous.body.is_empty() || current.body.is_empty() {
                continue;
            }
            if bodies_equal(&previous.body, &current.body) {
                self.store.emit(
                    current.line,
                    current.col,
                    "[info] Adjacent If/ElseIf branch body is identical to the previous \
                     branch; this often means a condition was copied and the body was \
                     left unchanged",
                    RULE,
                );
            }
        }
        let Some(previous) = branches.last() else {
            return;
        };
        if else_body.is_empty() || previous.body.is_empty() {
            return;
        }
        if let (Some(line), Some(column)) = (else_line, else_col) {
            if bodies_equal(&previous.body, else_body) {
                self.store.emit(
                    *line,
                    *column,
                    "[info] Else body is identical to the previous If/ElseIf branch; \
                     this often means the branch was copied and left unchanged",
                    RULE,
                );
            }
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for adjacent `If` / `ElseIf` branches whose bodies match.
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

fn bodies_equal(left: &[Stmt], right: &[Stmt]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| stmts_equal(a, b))
}

fn stmts_equal(left: &Stmt, right: &Stmt) -> bool {
    match (left, right) {
        (Stmt::VarDecl(a), Stmt::VarDecl(b)) => var_decls_equal(a, b),
        (
            Stmt::Assign {
                target: lt,
                op: lo,
                value: lv,
                ..
            },
            Stmt::Assign {
                target: rt,
                op: ro,
                value: rv,
                ..
            },
        ) => lo == ro && exprs_equal(lt, rt) && exprs_equal(lv, rv),
        (Stmt::Expr { value: lv, .. }, Stmt::Expr { value: rv, .. }) => exprs_equal(lv, rv),
        (Stmt::Return { value: lv, .. }, Stmt::Return { value: rv, .. }) => match (lv, rv) {
            (None, None) => true,
            (Some(a), Some(b)) => exprs_equal(a, b),
            _ => false,
        },
        (
            Stmt::If {
                branches: lb,
                else_body: le,
                else_line: ll,
                ..
            },
            Stmt::If {
                branches: rb,
                else_body: re,
                else_line: rl,
                ..
            },
        ) => {
            ll.is_some() == rl.is_some()
                && lb.len() == rb.len()
                && lb.iter().zip(rb).all(|(a, b)| {
                    exprs_equal(&a.condition, &b.condition) && bodies_equal(&a.body, &b.body)
                })
                && bodies_equal(le, re)
        }
        (
            Stmt::While {
                condition: lc,
                body: lb,
                ..
            },
            Stmt::While {
                condition: rc,
                body: rb,
                ..
            },
        ) => exprs_equal(lc, rc) && bodies_equal(lb, rb),
        (
            Stmt::LockGuard {
                kind: lk,
                names: ln,
                body: lb,
                else_body: le,
                else_line: ll,
                ..
            },
            Stmt::LockGuard {
                kind: rk,
                names: rn,
                body: rb,
                else_body: re,
                else_line: rl,
                ..
            },
        ) => {
            lk == rk
                && ll.is_some() == rl.is_some()
                && names_equal(ln, rn)
                && bodies_equal(lb, rb)
                && bodies_equal(le, re)
        }
        _ => false,
    }
}

fn var_decls_equal(left: &VariableDecl, right: &VariableDecl) -> bool {
    types_equal(&left.type_name, &right.type_name)
        && left.name.eq_ignore_ascii_case(&right.name)
        && left.is_conditional == right.is_conditional
        && option_names_equal(&left.requires_guard, &right.requires_guard)
        && match (&left.value, &right.value) {
            (None, None) => true,
            (Some(a), Some(b)) => exprs_equal(a, b),
            _ => false,
        }
}

fn types_equal(left: &TypeName, right: &TypeName) -> bool {
    left.is_array == right.is_array && left.name.eq_ignore_ascii_case(&right.name)
}

fn names_equal(left: &[String], right: &[String]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

fn option_names_equal(left: &Option<String>, right: &Option<String>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    }
}

fn exprs_equal(left: &Expr, right: &Expr) -> bool {
    match (left, right) {
        (Expr::Literal(a), Expr::Literal(b)) => literals_equal(a, b),
        (Expr::Identifier(a), Expr::Identifier(b)) => a.eq_ignore_ascii_case(b),
        (Expr::Self_, Expr::Self_) | (Expr::Parent, Expr::Parent) => true,
        (
            Expr::Binary {
                left: ll,
                op: lo,
                right: lr,
            },
            Expr::Binary {
                left: rl,
                op: ro,
                right: rr,
            },
        ) => lo == ro && exprs_equal(ll, rl) && exprs_equal(lr, rr),
        (
            Expr::Unary {
                op: lo,
                operand: lv,
            },
            Expr::Unary {
                op: ro,
                operand: rv,
            },
        ) => lo == ro && exprs_equal(lv, rv),
        (
            Expr::Call {
                callee: lc, args: la, ..
            },
            Expr::Call {
                callee: rc, args: ra, ..
            },
        ) => exprs_equal(lc, rc) && la.len() == ra.len() && la.iter().zip(ra).all(|(a, b)| exprs_equal(a, b)),
        (Expr::NamedArg { name: ln, value: lv }, Expr::NamedArg { name: rn, value: rv }) => {
            ln.eq_ignore_ascii_case(rn) && exprs_equal(lv, rv)
        }
        (
            Expr::Member {
                object: lo,
                property: lp,
            },
            Expr::Member {
                object: ro,
                property: rp,
            },
        ) => exprs_equal(lo, ro) && lp.eq_ignore_ascii_case(rp),
        (
            Expr::Index {
                object: lo,
                index: li,
            },
            Expr::Index {
                object: ro,
                index: ri,
            },
        ) => exprs_equal(lo, ro) && exprs_equal(li, ri),
        (
            Expr::Cast {
                value: lv,
                type_name: lt,
            },
            Expr::Cast {
                value: rv,
                type_name: rt,
            },
        ) => exprs_equal(lv, rv) && lt.eq_ignore_ascii_case(rt),
        (
            Expr::Is {
                value: lv,
                type_name: lt,
            },
            Expr::Is {
                value: rv,
                type_name: rt,
            },
        ) => exprs_equal(lv, rv) && lt.eq_ignore_ascii_case(rt),
        (
            Expr::NewArray {
                type_name: lt,
                size: ls,
            },
            Expr::NewArray {
                type_name: rt,
                size: rs,
            },
        ) => types_equal(lt, rt) && exprs_equal(ls, rs),
        (Expr::NewStruct { type_name: lt }, Expr::NewStruct { type_name: rt }) => {
            lt.eq_ignore_ascii_case(rt)
        }
        _ => false,
    }
}

fn literals_equal(left: &Literal, right: &Literal) -> bool {
    match (left, right) {
        (Literal::Int { value: a, .. }, Literal::Int { value: b, .. }) => a == b,
        (Literal::Float(a), Literal::Float(b)) => a == b,
        (Literal::String(a), Literal::String(b)) => a == b,
        (Literal::Bool(a), Literal::Bool(b)) => a == b,
        (Literal::None, Literal::None) => true,
        _ => false,
    }
}

#[cfg(test)]
#[path = "duplicate_conditional_body_tests.rs"]
mod tests;
