//! Flags, as an `[error]`, a call to `Utility.RandomInt` or
//! `Utility.RandomFloat` whose first two arguments both fold to
//! compile-time-constant numbers with the first not smaller than the
//! second, since both native functions require their first argument (the
//! minimum) to be smaller than their second (the maximum) — a call with the
//! bounds equal or reversed never produces any actual randomness.
//!
//! Like [`crate::division_by_zero`], this only folds an argument built
//! entirely from literals (optionally combined with arithmetic and unary
//! operators); an argument that depends on an identifier, a call, `Self`/
//! `Parent`, a member/index access, a cast, or a `new` array is left
//! unflagged rather than guessed at. A named argument is matched by its
//! position in the call, not by its name, the same way every other
//! argument-inspecting lint in this crate does.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, UnaryOp};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "invalid-random-range";

#[allow(dead_code)] // not dispatched from collect_diagnostics yet
pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::ast()
}

/// The native `Utility` singleton functions this lint checks, both only
/// ever called through that literal script name (see
/// `shared/rules/data/native-globals.yaml`), the same way [`crate::short_wait_interval`]
/// treats `Utility.Wait`.
const RANDOM_FUNCTIONS: &[&str] = &["RandomInt", "RandomFloat"];

/// Checks every call to `Utility.RandomInt`/`Utility.RandomFloat` in
/// `source`, flagging one whose first two arguments both fold to constant
/// numbers with the first not smaller than the second.
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
                    walk_expr(value, diagnostics);
                }
            }
            Stmt::Assign { target, value, .. } => {
                walk_expr(target, diagnostics);
                walk_expr(value, diagnostics);
            }
            Stmt::Expr { value, .. } => walk_expr(value, diagnostics),
            Stmt::Return {
                value: Some(value), ..
            } => {
                walk_expr(value, diagnostics);
            }
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch {
                    condition, body, ..
                } in branches
                {
                    walk_expr(condition, diagnostics);
                    check_body(body, diagnostics);
                }
                check_body(else_body, diagnostics);
            }
            Stmt::While {
                condition, body, ..
            } => {
                walk_expr(condition, diagnostics);
                check_body(body, diagnostics);
            }
        }
    }
}

fn walk_expr(expr: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    if let Expr::Call {
        callee,
        args,
        line,
        col,
    } = expr
    {
        if let Some(name) = matching_function(callee) {
            if let (Some(min_arg), Some(max_arg)) = (args.first(), args.get(1)) {
                if let (Some(min_literal), Some(max_literal)) = (
                    eval_const(unwrap_value(min_arg)),
                    eval_const(unwrap_value(max_arg)),
                ) {
                    if let (Some((min, _)), Some((max, _))) =
                        (as_number(&min_literal), as_number(&max_literal))
                    {
                        if min >= max {
                            diagnostics.push(Diagnostic {
                                line: *line,
                                column: *col,
                                message: format!(
                                    "[error] Utility.{name}({min}, {max}): the first argument \
                                     must be smaller than the second, or the call never \
                                     produces any actual randomness"
                                ),
                                rule: RULE,
                            });
                        }
                    }
                }
            }
            for arg in args {
                walk_expr(arg, diagnostics);
            }
            walk_expr(callee, diagnostics);
            return;
        }
        walk_expr(callee, diagnostics);
        for arg in args {
            walk_expr(arg, diagnostics);
        }
        return;
    }

    match expr {
        Expr::Binary { left, right, .. } => {
            walk_expr(left, diagnostics);
            walk_expr(right, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, diagnostics);
            walk_expr(index, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent | Expr::Call { .. } => {
        }
    }
}

/// Unwraps a call argument's underlying value, ignoring a `NamedArg` name.
fn unwrap_value(expr: &Expr) -> &Expr {
    match expr {
        Expr::NamedArg { value, .. } => value,
        other => other,
    }
}

/// Whether `callee` is a call to one of [`RANDOM_FUNCTIONS`], only matching
/// when explicitly qualified by the literal `Utility` script name, the same
/// way [`crate::short_wait_interval::matching_function`] treats
/// `Utility.Wait`.
fn matching_function(callee: &Expr) -> Option<&'static str> {
    let Expr::Member { object, property } = callee else {
        return None;
    };
    let name = *RANDOM_FUNCTIONS
        .iter()
        .find(|name| name.eq_ignore_ascii_case(property))?;
    let Expr::Identifier(qualifier) = object.as_ref() else {
        return None;
    };
    if !qualifier.eq_ignore_ascii_case("Utility") {
        return None;
    }
    Some(name)
}

/// Attempts to fold `expr` down to a single constant numeric [`Literal`],
/// the same way [`crate::division_by_zero`] does: returning `None` as soon
/// as any part of it depends on something that can't be known without
/// running the script.
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

/// Returns a folded literal's numeric value, alongside whether it was a
/// `Float` (as opposed to an `Int`) literal.
fn as_number(value: &Literal) -> Option<(f64, bool)> {
    match value {
        Literal::Int { value, .. } => Some((*value as f64, false)),
        Literal::Float(f) => Some((*f, true)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "invalid_random_range_tests.rs"]
mod tests;
