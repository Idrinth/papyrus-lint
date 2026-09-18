//! Flags a direct `==`/`!=` comparison between two `Float` values.
//!
//! Floating-point rounding error can make two values that are
//! conceptually "the same" (e.g. one computed via a different, but
//! mathematically equivalent, sequence of operations) compare unequal at
//! runtime, or vice versa. Comparing against a small epsilon (or an
//! inequality) is usually more robust than a direct `==`/`!=`.
//!
//! Like [`crate::numeric_comparison`], this works on the parsed AST (see
//! `papyrus_parser::types`) rather than raw tokens, so a script that fails
//! to parse simply isn't checked. Disabled by default: see
//! `crate::config::Rules::float_equality`.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Script, Stmt, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "float-equality";

/// Checks `source` for `Float`/`Float` `==`/`!=` comparisons.
pub fn check(ast: Option<&Script>) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };

    let mut env = TypeEnv::for_script(script);
    let mut diagnostics = Vec::new();

    for function in all_functions(script) {
        env.with_function_scope(function, |scoped| {
            check_body(&function.body, scoped, &mut diagnostics);
        });
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

fn check_body(body: &[Stmt], env: &TypeEnv, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    walk_expr(value, env, decl.line, diagnostics);
                }
            }
            Stmt::Assign {
                target,
                value,
                line,
                ..
            } => {
                walk_expr(target, env, *line, diagnostics);
                walk_expr(value, env, *line, diagnostics);
            }
            Stmt::Expr { value, line } => walk_expr(value, env, *line, diagnostics),
            Stmt::Return {
                value: Some(value),
                line,
            } => {
                walk_expr(value, env, *line, diagnostics);
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
                    walk_expr(condition, env, *line, diagnostics);
                    check_body(body, env, diagnostics);
                }
                check_body(else_body, env, diagnostics);
            }
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                walk_expr(condition, env, *line, diagnostics);
                check_body(body, env, diagnostics);
            }
        }
    }
}

/// Recursively walks `expr` looking for `Float`/`Float` `==`/`!=` comparisons.
///
/// `line` is the enclosing statement's line, since expressions don't carry
/// their own position in this AST.
fn walk_expr(expr: &Expr, env: &TypeEnv, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    if let Expr::Binary { left, op, right } = expr {
        if is_equality(*op) {
            check_comparison(left, right, env, line, diagnostics);
        }
        walk_expr(left, env, line, diagnostics);
        walk_expr(right, env, line, diagnostics);
        return;
    }

    match expr {
        Expr::Unary { operand, .. } => walk_expr(operand, env, line, diagnostics),
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, env, line, diagnostics);
            for arg in args {
                walk_expr(arg, env, line, diagnostics);
            }
        }
        Expr::Member { object, .. } => walk_expr(object, env, line, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, env, line, diagnostics);
            walk_expr(index, env, line, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, env, line, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, env, line, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, env, line, diagnostics),
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::Binary { .. } => {}
    }
}

fn is_equality(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
}

fn is_float(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("float")
}

/// Flags `left op right` when both sides are `Float`. Flagged as an
/// `[info]`.
fn check_comparison(
    left: &Expr,
    right: &Expr,
    env: &TypeEnv,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(left_ty) = infer_type(left, env) else {
        return;
    };
    let Some(right_ty) = infer_type(right, env) else {
        return;
    };

    if is_float(&left_ty) && is_float(&right_ty) {
        diagnostics.push(Diagnostic {
            line,
            column: 1,
            message: "[info] Comparing two Float values with '==' or '!=' directly; \
                      floating-point rounding error can make this comparison unreliable"
                .to_string(),
            rule: RULE,
        });
    }
}

#[cfg(test)]
#[path = "float_equality_tests.rs"]
mod tests;
