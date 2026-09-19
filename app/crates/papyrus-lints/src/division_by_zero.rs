//! Flags a `/` or `%` whose right-hand operand is a compile-time-constant
//! zero (e.g. `x / 0`, `x % 0.0`, `x / (1 - 1)`), since dividing (or taking
//! the modulo) by zero crashes the script at runtime.
//!
//! Like [`crate::static_condition`], this only folds an operand built
//! entirely from literals (optionally combined with arithmetic and unary
//! operators); a divisor that depends on an identifier, a call, `Self`/
//! `Parent`, a member/index access, a cast, or a `new` array is left
//! unflagged rather than guessed at.

use papyrus_parser::ast::{BinaryOp, Expr, Literal, UnaryOp};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "division-by-zero";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Expr::Binary { op, right, .. } = expr else {
            return;
        };
        if !matches!(op, BinaryOp::Div | BinaryOp::Mod) {
            return;
        }
        let Some(value) = eval_const(right) else {
            return;
        };
        if !is_zero(&value) {
            return;
        }
        let operator = if *op == BinaryOp::Div { "/" } else { "%" };
        self.store.emit(
            ctx.line,
            1,
            format!(
                "[warning] Right-hand side of `{operator}` is always zero; this \
                 divides by zero at runtime"
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every `/` and `%` expression in `source` and flags the ones whose
/// right-hand operand folds to a constant zero.
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

fn is_zero(value: &Literal) -> bool {
    matches!(value, Literal::Int { value: 0, .. })
        || matches!(value, Literal::Float(f) if *f == 0.0)
}

/// Attempts to fold `expr` down to a single constant numeric [`Literal`],
/// returning `None` as soon as any part of it depends on something that
/// can't be known without running the script (an identifier, a call,
/// `Self`/`Parent`, a member/index access, a cast, a `new` array, or a
/// division/modulo, since folding those would risk dividing by zero
/// itself).
fn eval_const(expr: &Expr) -> Option<Literal> {
    match expr {
        Expr::Literal(literal @ (Literal::Int { .. } | Literal::Float(_))) => Some(literal.clone()),
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => match eval_const(operand)? {
            Literal::Int { value, .. } => Some(Literal::int(-value)),
            Literal::Float(f) => Some(Literal::Float(-f)),
            _ => None,
        },
        Expr::Binary {
            left,
            op: op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul),
            right,
        } => {
            let (a, a_float) = as_number(&eval_const(left)?)?;
            let (b, b_float) = as_number(&eval_const(right)?)?;
            let result = match op {
                BinaryOp::Add => a + b,
                BinaryOp::Sub => a - b,
                BinaryOp::Mul => a * b,
                _ => unreachable!(),
            };
            Some(if a_float || b_float {
                Literal::Float(result)
            } else {
                Literal::int(result as i64)
            })
        }
        _ => None,
    }
}

fn as_number(value: &Literal) -> Option<(f64, bool)> {
    match value {
        Literal::Int { value, .. } => Some((*value as f64, false)),
        Literal::Float(f) => Some((*f, true)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "division_by_zero_tests.rs"]
mod tests;
