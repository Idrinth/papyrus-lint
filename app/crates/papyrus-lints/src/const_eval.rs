//! Shared compile-time folding of literal arithmetic.
//!
//! Several AST lints need to know whether an expression is a number (or an
//! `Int`) that can be determined without running the script — a divisor, a
//! `Wait` interval, a `new T[N]` size, an array index. They all used to
//! carry their own copy of this folder; keeping one implementation means
//! those lints cannot drift on what "a compile-time-constant number" is.
//!
//! [`eval_const`] folds `Int`/`Float` literals combined with unary `-` and
//! `+`/`-`/`*`. It deliberately does **not** fold `/` or `%` (those would
//! risk dividing by zero inside the folder itself) or anything that depends
//! on runtime state (an identifier, a call, `Self`/`Parent`, a member/index
//! access, a cast, a `new` array). [`eval_const_int`] is the `Int`-only
//! sibling: a `Float` anywhere in the tree is a miss, and arithmetic stays
//! on `i64` instead of going through `f64`.
//!
//! [`crate::static_condition`] has its own, broader folder (booleans,
//! strings, comparisons, `/`/`%` with a zero-divisor guard). That lint
//! needs those extra operators; the numeric lints must not start folding
//! them and silently changing which expressions they flag.

use papyrus_parser::ast::{BinaryOp, Expr, Literal, UnaryOp};

/// Attempts to fold `expr` down to a single constant numeric [`Literal`],
/// returning `None` as soon as any part of it depends on something that
/// can't be known without running the script (an identifier, a call,
/// `Self`/`Parent`, a member/index access, a cast, a `new` array, or a
/// division/modulo, since folding those would risk dividing by zero
/// itself).
pub(crate) fn eval_const(expr: &Expr) -> Option<Literal> {
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

/// Attempts to fold `expr` down to a single constant `Int`, returning `None`
/// as soon as any part of it depends on something that can't be known
/// without running the script (an identifier, a call, `Self`/`Parent`, a
/// member/index access, a cast, a `new` array, a `Float`, division, or
/// modulo).
///
/// Shared by [`crate::array_bounds`] and [`crate::unchecked_array_element`]
/// so both lints' notion of "the same array element" (e.g. `a[2]` and
/// `a[1 + 1]`) stays identical, and by [`crate::array_size_range`] for a
/// `new T[N]` size.
pub(crate) fn eval_const_int(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Literal(Literal::Int { value, .. }) => Some(*value),
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => eval_const_int(operand).map(|value| -value),
        Expr::Binary {
            left,
            op: op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul),
            right,
        } => {
            let left = eval_const_int(left)?;
            let right = eval_const_int(right)?;
            Some(match op {
                BinaryOp::Add => left + right,
                BinaryOp::Sub => left - right,
                BinaryOp::Mul => left * right,
                _ => unreachable!(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "const_eval_tests.rs"]
mod tests;
