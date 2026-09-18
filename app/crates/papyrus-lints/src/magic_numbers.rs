//! Flags a numeric literal used directly in an expression rather than
//! through a named constant, property, or local variable, as a
//! `[warning]`. Disabled by default (see [`crate::config::Rules::magic_numbers`]):
//! many existing scripts contain plenty of unremarkable literal numbers,
//! so a project has to opt in explicitly.
//!
//! `-1`, `0`, and `1` are never flagged, since they're near-universally
//! used directly (array bounds, increments, sentinel/empty checks) without
//! losing any clarity from being spelled out.
//!
//! A literal that's the entire value given to a declaration or assignment
//! (`Int kMaxTargets = 5`, later reassigned as `kMaxTargets = 6`) is left
//! alone too: naming it there already gives it the meaning this lint is
//! after. Only the bare literal itself is exempted this way; a literal
//! nested inside a more complex initializer (`Int kMaxTargets = 5 + 1`) is
//! still checked, since the declaration's name doesn't explain what either
//! operand means on its own.
//!
//! By default (the "loose" [`MagicNumbers`] mode), a numeric literal passed
//! as an argument to `Utility.Wait`, `RegisterForUpdate`,
//! `RegisterForSingleUpdate`, `RegisterForUpdateGameTime`, or
//! `RegisterForSingleUpdateGameTime` (see [`crate::short_wait_interval`],
//! which matches the same calls) is left unflagged too, since a hardcoded
//! interval there is both common and usually self-explanatory. The
//! "strict" mode also checks those arguments like any other.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, UnaryOp};
use serde::{Deserialize, Serialize};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "magic-numbers";

/// Whether this lint also checks the interval argument of a
/// `Utility.Wait`/`RegisterForUpdate`/`RegisterForSingleUpdate`/
/// `RegisterForUpdateGameTime`/`RegisterForSingleUpdateGameTime` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagicNumbers {
    /// The interval argument of a `Utility.Wait`/`RegisterFor*` call is
    /// never flagged.
    #[default]
    Loose,
    /// Every numeric literal is checked, including a `Utility.Wait`/
    /// `RegisterFor*` call's interval argument.
    Strict,
}

/// Checks `source` for numeric literals used directly rather than through
/// a named constant, property, or local variable, per `mode`.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::argument_types::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, external);
    let mode = config.magic_numbers;

    let Some(script) = ast else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for variable in &script.variables {
        if let Some(value) = &variable.value {
            walk_declaration_value(value, mode, variable.line, &mut diagnostics);
        }
    }
    for function in all_functions(script) {
        check_body(&function.body, mode, &mut diagnostics);
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

fn check_body(body: &[Stmt], mode: MagicNumbers, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    walk_declaration_value(value, mode, decl.line, diagnostics);
                }
            }
            Stmt::Assign {
                target,
                value,
                line,
                ..
            } => {
                walk_expr(target, mode, *line, false, diagnostics);
                walk_declaration_value(value, mode, *line, diagnostics);
            }
            Stmt::Expr { value, line } => walk_expr(value, mode, *line, false, diagnostics),
            Stmt::Return {
                value: Some(value),
                line,
            } => {
                walk_expr(value, mode, *line, false, diagnostics);
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
                    walk_expr(condition, mode, *line, false, diagnostics);
                    check_body(body, mode, diagnostics);
                }
                check_body(else_body, mode, diagnostics);
            }
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                walk_expr(condition, mode, *line, false, diagnostics);
                check_body(body, mode, diagnostics);
            }
        }
    }
}

/// Checks a declaration's or assignment's value expression: a bare numeric
/// literal (optionally negated) is exempt, since naming it there already
/// gives it the meaning this lint is after; anything else is walked
/// normally, so a literal nested inside a more complex initializer is
/// still checked.
fn walk_declaration_value(
    value: &Expr,
    mode: MagicNumbers,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if is_bare_number_literal(value) {
        return;
    }
    walk_expr(value, mode, line, false, diagnostics);
}

fn is_bare_number_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(Literal::Int { .. } | Literal::Float(_)) => true,
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => matches!(
            operand.as_ref(),
            Expr::Literal(Literal::Int { .. } | Literal::Float(_))
        ),
        _ => false,
    }
}

/// Recursively walks `expr`, flagging every numeric literal it finds
/// (subject to the ignored-value list and, in "loose" mode, the
/// `Wait`/`RegisterFor*` exemption). `wait_exempt` is threaded through
/// arithmetic composition (`Binary`, `Unary`) so a literal combined with
/// others inside an exempted call's argument stays exempt too, but resets
/// to `false` across a `Call`, `Member`, `Index`, `Cast`, or `NewArray`
/// boundary, since those introduce a value of their own rather than
/// composing the exempted argument's.
fn walk_expr(
    expr: &Expr,
    mode: MagicNumbers,
    line: usize,
    wait_exempt: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Literal(Literal::Int { value, .. }) => {
            if !wait_exempt {
                flag_int(*value, line, diagnostics);
            }
        }
        Expr::Literal(Literal::Float(f)) => {
            if !wait_exempt {
                flag_float(*f, line, diagnostics);
            }
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => match operand.as_ref() {
            Expr::Literal(Literal::Int { value, .. }) => {
                if !wait_exempt {
                    flag_int(-*value, line, diagnostics);
                }
            }
            Expr::Literal(Literal::Float(f)) => {
                if !wait_exempt {
                    flag_float(-*f, line, diagnostics);
                }
            }
            _ => walk_expr(operand, mode, line, wait_exempt, diagnostics),
        },
        Expr::Unary { operand, .. } => walk_expr(operand, mode, line, wait_exempt, diagnostics),
        Expr::Binary { left, right, .. } => {
            walk_expr(left, mode, line, wait_exempt, diagnostics);
            walk_expr(right, mode, line, wait_exempt, diagnostics);
        }
        Expr::Call { callee, args, .. } => {
            let exempt = mode == MagicNumbers::Loose
                && crate::short_wait_interval::matching_function(callee).is_some();
            for arg in args {
                let value_expr = match arg {
                    Expr::NamedArg { value, .. } => value.as_ref(),
                    other => other,
                };
                walk_expr(value_expr, mode, line, exempt, diagnostics);
            }
            walk_expr(callee, mode, line, false, diagnostics);
        }
        Expr::NamedArg { value, .. } => walk_expr(value, mode, line, wait_exempt, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, mode, line, false, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, mode, line, false, diagnostics);
            walk_expr(index, mode, line, false, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, mode, line, false, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, mode, line, false, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

fn is_ignored_int(value: i64) -> bool {
    matches!(value, -1..=1)
}

fn is_ignored_float(value: f64) -> bool {
    value == -1.0 || value == 0.0 || value == 1.0
}

fn flag_int(value: i64, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    if !is_ignored_int(value) {
        push(value.to_string(), line, diagnostics);
    }
}

fn flag_float(value: f64, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    if !is_ignored_float(value) {
        push(value.to_string(), line, diagnostics);
    }
}

fn push(display: String, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic {
        line,
        column: 1,
        message: format!(
            "[warning] Magic number {display}; extract it into a named constant, property, \
             or local variable so its meaning is clear"
        ),
        rule: RULE,
    });
}

#[cfg(test)]
#[path = "magic_numbers_tests.rs"]
mod tests;
