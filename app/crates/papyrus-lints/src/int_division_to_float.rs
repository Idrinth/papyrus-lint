//! Flags an `Int / Int` division whose result is then widened into a
//! `Float`-typed declaration, assignment, return, or argument.
//!
//! Papyrus evaluates `/` between two `Int`s as integer division *before*
//! any widening happens, so `Float x = 1 / 2` yields `0.0` rather than
//! `0.5` — the truncation already happened by the time the `Int` result
//! widens into the `Float` slot. Writing either operand as a `Float`
//! (`1.0 / 2`) avoids it. When both operands are compile-time-constant
//! integer literals *and* the division happens to divide evenly (e.g.
//! `72 / 8`), no truncation actually occurs, so that case is left
//! unflagged rather than reported as a false positive.
//!
//! Like [`crate::float_int_conversion`], this needs a value's inferred
//! type, so it works on the parsed AST (see `papyrus_parser::types`)
//! rather than raw tokens. Scripts that fail to parse simply aren't
//! checked, the same way a lexer failure short-circuits the token-based
//! lints.

use papyrus_parser::ast::{BinaryOp, Expr, Literal, TypeName, UnaryOp};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::type_flow::{Slot, TypeFlowLint};
use crate::visitor::{LintVisitor, Store};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "int-division-to-float";

#[derive(Default)]
struct IntDivisionToFloat;

impl TypeFlowLint for IntDivisionToFloat {
    fn check(
        &mut self,
        target_type: &TypeName,
        value: &Expr,
        env: &TypeEnv,
        slot: Slot<'_>,
        line: usize,
        store: &mut Store,
    ) {
        if !is_float(target_type) {
            return;
        }
        let context = match slot {
            Slot::Declaration { name } => format!("assigned to Float variable '{name}'"),
            Slot::Assignment { target } => format!("assigned to Float {}", describe_target(target)),
            Slot::Return { function } => format!("returned from Float function '{function}'"),
            Slot::Argument { parameter, function } => {
                format!("passed as Float parameter '{parameter}' of function '{function}'")
            }
        };
        for _ in 0..count_int_divisions(value, env) {
            store.emit(line, 1, format!("[warning] {MESSAGE} ({context})"), RULE);
        }
    }
}

pub fn visitor() -> LintVisitor {
    crate::type_flow::visitor::<IntDivisionToFloat>()
}

const MESSAGE: &str = "Int/Int division truncates its result before it widens into a Float; \
                        write one operand as a Float (e.g. 1.0 / x) to keep the fractional result";

/// Checks `source` for an `Int / Int` division whose result is widened
/// into a Float without either operand already being a Float.
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

fn is_int(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("int")
}

fn is_float(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("float")
}

fn is_int_expr(expr: &Expr, env: &TypeEnv) -> bool {
    infer_type(expr, env).is_some_and(|value_type| is_int(&value_type))
}

/// Counts every `Int / Int` division reachable from `expr` through
/// arithmetic operators, unary negation, casts, and named arguments —
/// the constructs that keep `expr` part of the same arithmetic result —
/// without crossing into a nested call's own arguments, an index/member
/// access, or a `new` array, which each establish their own independent
/// type context unrelated to whatever `expr` as a whole widens into.
fn count_int_divisions(expr: &Expr, env: &TypeEnv) -> usize {
    let mut count = 0;
    collect_int_divisions(expr, env, &mut count);
    count
}

fn collect_int_divisions(expr: &Expr, env: &TypeEnv, count: &mut usize) {
    if let Expr::Binary {
        left,
        op: BinaryOp::Div,
        right,
    } = expr
    {
        if is_int_expr(left, env) && is_int_expr(right, env) && !divides_evenly(left, right) {
            *count += 1;
        }
        collect_int_divisions(left, env, count);
        collect_int_divisions(right, env, count);
        return;
    }

    match expr {
        Expr::Binary { left, right, .. } => {
            collect_int_divisions(left, env, count);
            collect_int_divisions(right, env, count);
        }
        Expr::Unary { operand, .. } => collect_int_divisions(operand, env, count),
        Expr::Cast { value, .. } | Expr::Is { value, .. } => collect_int_divisions(value, env, count),
        Expr::NamedArg { value, .. } => collect_int_divisions(value, env, count),
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::Call { .. }
        | Expr::Member { .. }
        | Expr::Index { .. }
        | Expr::NewArray { .. }
        | Expr::NewStruct { .. } => {}
    }
}

/// True when `left / right` is a division between two compile-time-constant
/// integer literals that happens to divide evenly, so widening its result
/// into a Float loses nothing — e.g. `72 / 8` (which is `9`, not truncated
/// from something like `9.14...`). Mirrors the conservative folding in
/// `division_by_zero`: only literals, negation, and `+`/`-`/`*` of
/// already-folded operands are folded, never another division/modulo, and
/// anything that depends on an identifier, a call, `Self`/`Parent`, a
/// member/index access, a cast, or a `new` array is left unresolved (and so
/// still flagged, since we can't tell whether it divides evenly).
fn divides_evenly(left: &Expr, right: &Expr) -> bool {
    let Some(a) = fold_int_literal(left) else {
        return false;
    };
    let Some(b) = fold_int_literal(right) else {
        return false;
    };
    b != 0 && a % b == 0
}

fn fold_int_literal(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Literal(Literal::Int { value, .. }) => Some(*value),
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => fold_int_literal(operand)?.checked_neg(),
        Expr::Binary {
            left,
            op: op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul),
            right,
        } => {
            let a = fold_int_literal(left)?;
            let b = fold_int_literal(right)?;
            match op {
                BinaryOp::Add => a.checked_add(b),
                BinaryOp::Sub => a.checked_sub(b),
                BinaryOp::Mul => a.checked_mul(b),
                _ => unreachable!(),
            }
        }
        _ => None,
    }
}

fn describe_target(target: &Expr) -> String {
    match target {
        Expr::Identifier(name) => format!("variable '{name}'"),
        Expr::Member { property, .. } => format!("property '{property}'"),
        Expr::Index { .. } => "array element".to_string(),
        _ => "target".to_string(),
    }
}

#[cfg(test)]
#[path = "int_division_to_float_tests.rs"]
mod tests;
