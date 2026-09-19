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

use papyrus_parser::ast::{BinaryOp, Expr, IfBranch, Literal, Stmt, UnaryOp};

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

/// Attempts to fold `expr` down to a single constant [`Literal`], returning
/// `None` as soon as any part of it depends on something that can't be
/// known without running the script (an identifier, a call, `Self`/
/// `Parent`, a member/index access, a cast, or a `new` array).
fn eval_const(expr: &Expr) -> Option<Literal> {
    match expr {
        Expr::Literal(literal) => Some(literal.clone()),
        Expr::Unary { op, operand } => eval_unary(*op, &eval_const(operand)?),
        Expr::Binary { left, op, right } => {
            eval_binary(&eval_const(left)?, *op, &eval_const(right)?)
        }
        Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::Call { .. }
        | Expr::Member { .. }
        | Expr::Index { .. }
        | Expr::Cast { .. }
        | Expr::NewArray { .. }
        | Expr::NamedArg { .. } => None,
    }
}

fn truthy(value: &Literal) -> bool {
    match value {
        Literal::Bool(b) => *b,
        Literal::Int { value, .. } => *value != 0,
        Literal::Float(f) => *f != 0.0,
        Literal::String(s) => !s.is_empty(),
        Literal::None => false,
    }
}

fn eval_unary(op: UnaryOp, value: &Literal) -> Option<Literal> {
    match op {
        UnaryOp::Not => Some(Literal::Bool(!truthy(value))),
        UnaryOp::Neg => match value {
            Literal::Int { value, .. } => Some(Literal::int(-value)),
            Literal::Float(f) => Some(Literal::Float(-f)),
            _ => None,
        },
    }
}

/// A numeric literal's value, promoted to `f64` so `Int`/`Float` operands
/// can be combined uniformly; remembers whether either side was a `Float`
/// so arithmetic results can be folded back to the right literal kind.
fn as_number(value: &Literal) -> Option<(f64, bool)> {
    match value {
        Literal::Int { value, .. } => Some((*value as f64, false)),
        Literal::Float(f) => Some((*f, true)),
        _ => None,
    }
}

fn eval_binary(left: &Literal, op: BinaryOp, right: &Literal) -> Option<Literal> {
    match op {
        BinaryOp::And => Some(Literal::Bool(truthy(left) && truthy(right))),
        BinaryOp::Or => Some(Literal::Bool(truthy(left) || truthy(right))),
        BinaryOp::Eq => Some(Literal::Bool(literal_eq(left, right)?)),
        BinaryOp::NotEq => Some(Literal::Bool(!literal_eq(left, right)?)),
        BinaryOp::Add
            if matches!(left, Literal::String(_)) || matches!(right, Literal::String(_)) =>
        {
            match (left, right) {
                (Literal::String(a), Literal::String(b)) => {
                    Some(Literal::String(format!("{a}{b}")))
                }
                _ => None,
            }
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
            let (a, a_float) = as_number(left)?;
            let (b, b_float) = as_number(right)?;
            let result = match op {
                BinaryOp::Add => a + b,
                BinaryOp::Sub => a - b,
                BinaryOp::Mul => a * b,
                BinaryOp::Div => {
                    if b == 0.0 {
                        return None;
                    }
                    a / b
                }
                BinaryOp::Mod => {
                    if b == 0.0 {
                        return None;
                    }
                    a % b
                }
                _ => unreachable!(),
            };
            Some(if a_float || b_float {
                Literal::Float(result)
            } else {
                Literal::int(result as i64)
            })
        }
        BinaryOp::Gt | BinaryOp::Lt | BinaryOp::GtEq | BinaryOp::LtEq => {
            let (a, _) = as_number(left)?;
            let (b, _) = as_number(right)?;
            Some(Literal::Bool(match op {
                BinaryOp::Gt => a > b,
                BinaryOp::Lt => a < b,
                BinaryOp::GtEq => a >= b,
                BinaryOp::LtEq => a <= b,
                _ => unreachable!(),
            }))
        }
    }
}

fn literal_eq(left: &Literal, right: &Literal) -> Option<bool> {
    match (left, right) {
        (Literal::String(a), Literal::String(b)) => Some(a == b),
        (Literal::Bool(a), Literal::Bool(b)) => Some(a == b),
        (Literal::None, Literal::None) => Some(true),
        (Literal::None, _) | (_, Literal::None) => Some(false),
        _ => {
            let (a, _) = as_number(left)?;
            let (b, _) = as_number(right)?;
            Some(a == b)
        }
    }
}

#[cfg(test)]
#[path = "static_condition_tests.rs"]
mod tests;
