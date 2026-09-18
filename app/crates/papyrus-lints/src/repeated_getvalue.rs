//! Flags a `GlobalVariable.GetValue()` call repeated on the same receiver
//! across the conditions of a single `If`/`ElseIf` chain (e.g. `If
//! gv.GetValue() == 1.0` / `ElseIf gv.GetValue() == 2.0`), since none of the
//! chain's earlier branch bodies run before a later condition is evaluated
//! (only the first matching branch's body ever executes), so the global's
//! value can't have changed between those reads — it can safely be read
//! into a local variable once ahead of the chain instead of being
//! re-fetched on every branch. Disabled by default: see
//! [`crate::config::Rules::repeated_getvalue`].
//!
//! Like the "Slow function usage" lint (`slow_functions`), a call's
//! receiver can't generally be resolved back to a `GlobalVariable`-typed
//! script (the lexer/parser have no type/symbol resolution), so this
//! matches by method name alone (`GetValue`, case-insensitively, with no
//! arguments) rather than requiring the receiver's declared type — the only
//! native method named `GetValue` (see `rules/native-methods.yaml`) belongs
//! to `GlobalVariable`.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "repeated-getvalue";

/// Checks every `If`/`ElseIf` chain in `source` for a `GetValue()` call
/// repeated on the same receiver across its conditions.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::argument_types::ExternalSignatures,
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
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                check_branches(branches, diagnostics);
                for IfBranch { body, .. } in branches {
                    check_body(body, diagnostics);
                }
                check_body(else_body, diagnostics);
            }
            Stmt::While { body, .. } => check_body(body, diagnostics),
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }
}

/// Walks `branches`' conditions in order, flagging every `GetValue()` call
/// whose receiver expression already appeared in an earlier call — whether
/// that earlier call was in a previous branch's condition or earlier in the
/// same one (e.g. `gv.GetValue() == 1.0 || gv.GetValue() == 2.0`).
fn check_branches(branches: &[IfBranch], diagnostics: &mut Vec<Diagnostic>) {
    let mut seen_receivers: Vec<&Expr> = Vec::new();
    for IfBranch { condition, .. } in branches {
        for (receiver, line, column) in get_value_calls(condition) {
            if seen_receivers.contains(&receiver) {
                diagnostics.push(Diagnostic {
                    line,
                    column,
                    message: "[info] This GlobalVariable's GetValue() is already read earlier in this If/ElseIf chain; read it into a local variable once ahead of the chain instead of calling GetValue() again on every branch".to_string(),
                    rule: RULE,
                });
            } else {
                seen_receivers.push(receiver);
            }
        }
    }
}

/// Collects every `receiver.GetValue()` call within `expr`, alongside the
/// call's own line/column, regardless of where in the expression tree it
/// sits (combined with `&&`/`||`, negated, nested in another call's
/// arguments, ...).
fn get_value_calls(expr: &Expr) -> Vec<(&Expr, usize, usize)> {
    let mut calls = Vec::new();
    collect_get_value_calls(expr, &mut calls);
    calls
}

fn collect_get_value_calls<'a>(expr: &'a Expr, out: &mut Vec<(&'a Expr, usize, usize)>) {
    if let Expr::Call {
        callee,
        args,
        line,
        col,
    } = expr
    {
        if args.is_empty() {
            if let Expr::Member { object, property } = callee.as_ref() {
                if property.eq_ignore_ascii_case("GetValue") {
                    out.push((object.as_ref(), *line, *col));
                }
            }
        }
    }

    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
        Expr::Binary { left, right, .. } => {
            collect_get_value_calls(left, out);
            collect_get_value_calls(right, out);
        }
        Expr::Unary { operand, .. } => collect_get_value_calls(operand, out),
        Expr::Call { callee, args, .. } => {
            collect_get_value_calls(callee, out);
            for arg in args {
                collect_get_value_calls(arg, out);
            }
        }
        Expr::NamedArg { value, .. } => collect_get_value_calls(value, out),
        Expr::Member { object, .. } => collect_get_value_calls(object, out),
        Expr::Index { object, index } => {
            collect_get_value_calls(object, out);
            collect_get_value_calls(index, out);
        }
        Expr::Cast { value, .. } => collect_get_value_calls(value, out),
        Expr::NewArray { size, .. } => collect_get_value_calls(size, out),
    }
}

#[cfg(test)]
#[path = "repeated_getvalue_tests.rs"]
mod tests;
