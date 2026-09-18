//! Flags a `/` or `%` whose right-hand operand is a compile-time-constant
//! zero (e.g. `x / 0`, `x % 0.0`, `x / (1 - 1)`), since dividing (or taking
//! the modulo) by zero crashes the script at runtime.
//!
//! Like [`crate::static_condition`], this only folds an operand built
//! entirely from literals (optionally combined with arithmetic and unary
//! operators); a divisor that depends on an identifier, a call, `Self`/
//! `Parent`, a member/index access, a cast, or a `new` array is left
//! unflagged rather than guessed at.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, UnaryOp};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "division-by-zero";

/// Checks every `/` and `%` expression in `source` and flags the ones whose
/// right-hand operand folds to a constant zero.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config, external);

    let Some(script) = ast else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        check_body(&function.body, &mut diagnostics);
    }
    diagnostics
}

fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

fn check_body(body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    walk_expr(value, decl.line, diagnostics);
                }
            }
            Stmt::Assign {
                target,
                value,
                line,
                ..
            } => {
                walk_expr(target, *line, diagnostics);
                walk_expr(value, *line, diagnostics);
            }
            Stmt::Expr { value, line } => walk_expr(value, *line, diagnostics),
            Stmt::Return {
                value: Some(value),
                line,
            } => {
                walk_expr(value, *line, diagnostics);
            }
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch {
                    condition,
                    body,
                    line,
                    ..
                } in branches
                {
                    walk_expr(condition, *line, diagnostics);
                    check_body(body, diagnostics);
                }
                check_body(else_body, diagnostics);
            }
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                walk_expr(condition, *line, diagnostics);
                check_body(body, diagnostics);
            }
        }
    }
}

/// Recursively walks `expr` looking for `/`/`%` by a constant zero.
///
/// `line` is the enclosing statement's line, since expressions don't carry
/// their own position in this AST.
fn walk_expr(expr: &Expr, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    if let Expr::Binary { left, op, right } = expr {
        if matches!(op, BinaryOp::Div | BinaryOp::Mod) {
            if let Some(value) = eval_const(right) {
                if is_zero(&value) {
                    let operator = if *op == BinaryOp::Div { "/" } else { "%" };
                    diagnostics.push(Diagnostic {
                        line,
                        column: 1,
                        message: format!(
                            "[warning] Right-hand side of `{operator}` is always zero; this \
                             divides by zero at runtime"
                        ),
                        rule: RULE,
                    });
                }
            }
        }
        walk_expr(left, line, diagnostics);
        walk_expr(right, line, diagnostics);
        return;
    }

    match expr {
        Expr::Unary { operand, .. } => walk_expr(operand, line, diagnostics),
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, line, diagnostics);
            for arg in args {
                walk_expr(arg, line, diagnostics);
            }
        }
        Expr::Member { object, .. } => walk_expr(object, line, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, line, diagnostics);
            walk_expr(index, line, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, line, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, line, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, line, diagnostics),
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::Binary { .. } => {}
    }
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
