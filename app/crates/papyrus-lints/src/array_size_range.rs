//! Flags, as an `[error]`, a `new <Type>[<N>]` array creation whose literal
//! `N` falls outside the range Papyrus allows for a script-created array
//! (`0` to `128`), since a size outside that range is almost never
//! intended: per the CreationKit wiki's [Arrays
//! (Papyrus)](https://ck.uesp.net/wiki/Arrays_(Papyrus)) page, an array
//! created by a script via `New` (or grown with `Add()`) is hard-capped at
//! 128 elements by the engine, and a negative size makes no sense at all.
//! That cap does not apply to an array returned by a native function or to
//! an editor-populated array `Property`, since neither is created this way.
//!
//! This is a deliberately small, standalone check split out from
//! [`crate::array_bounds`]: unlike that lint's flow-sensitive tracking of
//! which local variable currently holds an array of which size, this one
//! only ever looks at a `new <Type>[<N>]` expression's own literal size, so
//! it has no state to track and nothing else can disable or narrow it.

use papyrus_parser::ast::{BinaryOp, Expr, IfBranch, Literal, Stmt, UnaryOp};

use crate::none_form_usage::all_functions;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "array-size-range";

#[allow(dead_code)] // not dispatched from collect_diagnostics yet
pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::ast()
}

/// The maximum number of elements an array created by a script (via `New`
/// or grown with `Add()`) can hold, per the CreationKit wiki's [Arrays
/// (Papyrus)](https://ck.uesp.net/wiki/Arrays_(Papyrus)) page.
const MAX_NEW_ARRAY_SIZE: i64 = 128;

/// Checks every function/event in `source` for a `new <Type>[<N>]` whose
/// literal `N` falls outside `0..=128`.
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
        walk_body(&function.body, &mut diagnostics);
    }
    diagnostics
}

fn walk_body(body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    check_expr(value, diagnostics, decl.line);
                }
            }
            Stmt::Assign {
                target,
                value,
                line,
                ..
            } => {
                check_expr(target, diagnostics, *line);
                check_expr(value, diagnostics, *line);
            }
            Stmt::Expr { value, line } => check_expr(value, diagnostics, *line),
            Stmt::Return {
                value: Some(value),
                line,
            } => check_expr(value, diagnostics, *line),
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => handle_if(branches, else_body, diagnostics),
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                check_expr(condition, diagnostics, *line);
                walk_body(body, diagnostics);
            }
        }
    }
}

fn handle_if(branches: &[IfBranch], else_body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    for branch in branches {
        check_expr(&branch.condition, diagnostics, branch.line);
        walk_body(&branch.body, diagnostics);
    }
    walk_body(else_body, diagnostics);
}

/// Recursively checks `expr` for a `new <Type>[<N>]` whose literal `N`
/// falls outside `0..=128`, recursing into every sub-expression so a
/// `new` nested anywhere within a larger expression (an argument, an
/// index, ...) is still found.
fn check_expr(expr: &Expr, diagnostics: &mut Vec<Diagnostic>, line: usize) {
    match expr {
        Expr::Index { object, index } => {
            check_expr(object, diagnostics, line);
            check_expr(index, diagnostics, line);
        }
        Expr::Member { object, .. } => check_expr(object, diagnostics, line),
        Expr::Call { callee, args, .. } => {
            check_expr(callee, diagnostics, line);
            for arg in args {
                check_expr(arg, diagnostics, line);
            }
        }
        Expr::Binary { left, right, .. } => {
            check_expr(left, diagnostics, line);
            check_expr(right, diagnostics, line);
        }
        Expr::Unary { operand, .. } => check_expr(operand, diagnostics, line),
        Expr::Cast { value, .. } => check_expr(value, diagnostics, line),
        Expr::NewArray { size, .. } => {
            check_expr(size, diagnostics, line);
            if let Some(literal_size) = eval_const_int(size) {
                if !(0..=MAX_NEW_ARRAY_SIZE).contains(&literal_size) {
                    diagnostics.push(Diagnostic {
                        line,
                        column: 1,
                        message: format!(
                            "[error] Array size {literal_size} is outside the range Papyrus \
                             allows (0 to {MAX_NEW_ARRAY_SIZE}) for an array created with `new`"
                        ),
                        rule: RULE,
                    });
                }
            }
        }
        Expr::NamedArg { value, .. } => check_expr(value, diagnostics, line),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Attempts to fold `expr` down to a single constant `Int`, returning `None`
/// as soon as any part of it depends on something that can't be known
/// without running the script (an identifier, a call, `Self`/`Parent`, a
/// member/index access, a cast, a `new` array, a `Float`, division, or
/// modulo).
fn eval_const_int(expr: &Expr) -> Option<i64> {
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
#[path = "array_size_range_tests.rs"]
mod tests;
