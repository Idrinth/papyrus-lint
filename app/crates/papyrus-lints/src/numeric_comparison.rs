//! Flags implicit comparisons between different numeric types: an `Int`
//! value compared against a `Float` value (`==`, `!=`, `<`, `<=`, `>`, `>=`)
//! without an explicit cast making the comparison exact.
//!
//! Papyrus implicitly widens the `Int` side to `Float` for such a
//! comparison, which can produce surprising results (particularly for
//! `==`/`!=`) once floating-point precision is involved.
//!
//! Like the other type-aware lints in this crate, this one works on the
//! parsed AST (see `papyrus_parser::types`) rather than raw tokens. Scripts
//! that fail to parse simply aren't checked.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Script, Stmt, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "numeric-comparison";

pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::from_ast(lint_issues)
}

/// Checks `source` for `Int`/`Float` comparisons that aren't already made
/// exact by an explicit cast.
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

fn lint_issues(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut dyn crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config, external);

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

/// Recursively walks `expr` looking for `Int`/`Float` comparisons.
///
/// `line` is the enclosing statement's line, since expressions don't carry
/// their own position in this AST.
fn walk_expr(expr: &Expr, env: &TypeEnv, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    if let Expr::Binary { left, op, right } = expr {
        if is_comparison(*op) {
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

fn is_comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Gt
            | BinaryOp::Lt
            | BinaryOp::GtEq
            | BinaryOp::LtEq
    )
}

fn is_int(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("int")
}

fn is_float(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("float")
}

/// Flags `left op right` when one side is `Int` and the other `Float`.
/// Flagged as a `[warning]`.
///
/// A side's inferred type already reflects any explicit cast it carries
/// (`someFloat as Int` infers as `Int`), so comparing the plain inferred
/// types is enough to let explicit casts through.
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

    let mismatched =
        (is_int(&left_ty) && is_float(&right_ty)) || (is_float(&left_ty) && is_int(&right_ty));
    if mismatched {
        diagnostics.push(Diagnostic {
            line,
            column: 1,
            message: "[warning] Comparison between Int and Float without an explicit cast; \
                      floating-point precision may make this inexact"
                .to_string(),
            rule: RULE,
        });
    }
}

#[cfg(test)]
#[path = "numeric_comparison_tests.rs"]
mod tests;
