//! Flags a call to `Utility.Wait`, `RegisterForUpdate`,
//! `RegisterForSingleUpdate`, `RegisterForUpdateGameTime`, or
//! `RegisterForSingleUpdateGameTime` whose interval argument folds to a
//! compile-time-constant number below a configurable minimum (`Config`'s
//! `min_wait_interval`, default `0.1`), since an interval that short runs
//! far more often than is typically useful and can add up to meaningful
//! performance overhead.
//!
//! Like [`crate::division_by_zero`], this only folds an argument built
//! entirely from literals (optionally combined with arithmetic and unary
//! operators); an argument that depends on an identifier, a call, `Self`/
//! `Parent`, a member/index access, a cast, or a `new` array is left
//! unflagged rather than guessed at. Always reported as a `[warning]`,
//! regardless of how far below the minimum the value is.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, UnaryOp};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "short-wait-interval";

#[allow(dead_code)] // not dispatched from collect_diagnostics yet
pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::ast()
}

pub(crate) struct WaitFunction {
    pub(crate) name: &'static str,
    /// Whether `name` is a native singleton function (only `Utility.Wait`)
    /// always called through its literal script name, rather than an
    /// instance method (the `RegisterFor*` family) reachable unqualified
    /// or through any receiver. See `check` for how this is used.
    global: bool,
}

pub(crate) const WAIT_FUNCTIONS: &[WaitFunction] = &[
    WaitFunction {
        name: "Wait",
        global: true,
    },
    WaitFunction {
        name: "RegisterForUpdate",
        global: false,
    },
    WaitFunction {
        name: "RegisterForSingleUpdate",
        global: false,
    },
    WaitFunction {
        name: "RegisterForUpdateGameTime",
        global: false,
    },
    WaitFunction {
        name: "RegisterForSingleUpdateGameTime",
        global: false,
    },
];

/// Checks every call to `Utility.Wait`/`RegisterForUpdate`/
/// `RegisterForSingleUpdate`/`RegisterForUpdateGameTime`/
/// `RegisterForSingleUpdateGameTime` in `source`, flagging one whose sole
/// argument folds to a constant number below `minimum`.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, external);
    let minimum = config.min_wait_interval;

    let Some(script) = ast else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        check_body(&function.body, minimum, &mut diagnostics);
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

fn check_body(body: &[Stmt], minimum: f64, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    walk_expr(value, minimum, diagnostics);
                }
            }
            Stmt::Assign { target, value, .. } => {
                walk_expr(target, minimum, diagnostics);
                walk_expr(value, minimum, diagnostics);
            }
            Stmt::Expr { value, .. } => walk_expr(value, minimum, diagnostics),
            Stmt::Return {
                value: Some(value), ..
            } => {
                walk_expr(value, minimum, diagnostics);
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
                    walk_expr(condition, minimum, diagnostics);
                    check_body(body, minimum, diagnostics);
                }
                check_body(else_body, minimum, diagnostics);
            }
            Stmt::While {
                condition, body, ..
            } => {
                walk_expr(condition, minimum, diagnostics);
                check_body(body, minimum, diagnostics);
            }
        }
    }
}

fn walk_expr(expr: &Expr, minimum: f64, diagnostics: &mut Vec<Diagnostic>) {
    if let Expr::Call {
        callee,
        args,
        line,
        col,
    } = expr
    {
        if let Some(function) = matching_function(callee) {
            if let Some(argument) = args.first() {
                let value_expr = match argument {
                    Expr::NamedArg { value, .. } => value,
                    other => other,
                };
                if let Some(value) = eval_const(value_expr) {
                    if let Some((number, _)) = as_number(&value) {
                        if number < minimum {
                            diagnostics.push(Diagnostic {
                                line: *line,
                                column: *col,
                                message: format!(
                                    "[warning] {}({number}) is below the configured minimum \
                                     interval of {minimum}; an interval that short runs far \
                                     more often than typically useful and can add up to \
                                     meaningful performance overhead",
                                    function.name
                                ),
                                rule: RULE,
                            });
                        }
                    }
                }
                walk_expr(value_expr, minimum, diagnostics);
            }
            for arg in args.iter().skip(1) {
                walk_expr(arg, minimum, diagnostics);
            }
            walk_expr(callee, minimum, diagnostics);
            return;
        }
        walk_expr(callee, minimum, diagnostics);
        for arg in args {
            walk_expr(arg, minimum, diagnostics);
        }
        return;
    }

    match expr {
        Expr::Binary { left, right, .. } => {
            walk_expr(left, minimum, diagnostics);
            walk_expr(right, minimum, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, minimum, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, minimum, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, minimum, diagnostics);
            walk_expr(index, minimum, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, minimum, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, minimum, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, minimum, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent | Expr::Call { .. } => {
        }
    }
}

/// Whether `callee` is a call to one of [`WAIT_FUNCTIONS`], honoring each
/// rule's `global` flag the same way `forbidden_functions`/`slow_functions`
/// do: a `global` rule (`Utility.Wait`) only matches when explicitly
/// qualified by that literal script name, while a non-`global` rule (the
/// `RegisterFor*` family) matches unqualified or through any receiver.
///
/// Also used by [`crate::magic_numbers`] to exempt these same calls'
/// interval arguments from its "loose" mode.
pub(crate) fn matching_function(callee: &Expr) -> Option<&'static WaitFunction> {
    match callee {
        Expr::Identifier(name) => WAIT_FUNCTIONS
            .iter()
            .find(|function| !function.global && function.name.eq_ignore_ascii_case(name)),
        Expr::Member { object, property } => {
            let function = WAIT_FUNCTIONS
                .iter()
                .find(|function| function.name.eq_ignore_ascii_case(property))?;
            if function.global {
                let Expr::Identifier(qualifier) = object.as_ref() else {
                    return None;
                };
                if !qualifier.eq_ignore_ascii_case("Utility") {
                    return None;
                }
            }
            Some(function)
        }
        _ => None,
    }
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
#[path = "short_wait_interval_tests.rs"]
mod tests;
