//! Shared compile-time folding of literal expressions.
//!
//! Several AST lints need to know whether an expression's value can be
//! determined without running the script — a divisor, a `Wait` interval, a
//! `new T[N]` size, an array index, an `If` condition. They all share this
//! folder so they cannot drift on what "a compile-time constant" is.
//!
//! [`eval_const`] folds any expression built entirely from literals combined
//! with arithmetic (`+`/`-`/`*`/`/`/`%`, including string `+`), comparison,
//! logical, and unary operators. `/` and `%` by a constant zero are a miss
//! (the folder must not divide by zero itself); anything that depends on
//! runtime state (an identifier, a call, `Self`/`Parent`, a member/index
//! access, a cast, a `new` array) is a miss too. [`eval_const_int`] is the
//! `Int`-only view of the same folder: a result that isn't an `Int` (a
//! `Float`, a `Bool`, …) is a miss.

use papyrus_parser::ast::{BinaryOp, Expr, Literal, UnaryOp};

/// Attempts to fold `expr` down to a single constant [`Literal`], returning
/// `None` as soon as any part of it depends on something that can't be
/// known without running the script (an identifier, a call, `Self`/
/// `Parent`, a member/index access, a cast, or a `new` array).
pub(crate) fn eval_const(expr: &Expr) -> Option<Literal> {
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
        | Expr::Is { .. }
        | Expr::NewArray { .. }
        | Expr::NewStruct { .. }
        | Expr::NamedArg { .. } => None,
    }
}

/// Papyrus truthiness of a folded literal, matching how an `If` condition
/// would treat it at runtime.
pub(crate) fn truthy(value: &Literal) -> bool {
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
pub(crate) fn as_number(value: &Literal) -> Option<(f64, bool)> {
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

/// Attempts to fold `expr` down to a single constant `Int`, returning `None`
/// as soon as [`eval_const`] cannot fold it, or the folded result isn't an
/// `Int` (a `Float`, a `Bool`, a string, …).
///
/// Shared by [`crate::array_bounds`] and [`crate::unchecked_array_element`]
/// so both lints' notion of "the same array element" (e.g. `a[2]` and
/// `a[1 + 1]`) stays identical, and by [`crate::array_size_range`] for a
/// `new T[N]` size.
pub(crate) fn eval_const_int(expr: &Expr) -> Option<i64> {
    match eval_const(expr)? {
        Literal::Int { value, .. } => Some(value),
        _ => None,
    }
}

#[cfg(test)]
#[path = "const_eval_tests.rs"]
mod tests;
