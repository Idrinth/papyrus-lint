//! Flags a `SetValue`/`SetValueInt` call standing alone as its own
//! statement, directly in a `While` loop's body or nested inside an
//! `If`/`ElseIf`/`Else` within it (not a further nested `While`, which is
//! checked on its own), as a `[warning]`, since a call there runs on every
//! iteration (or every iteration that reaches it) the loop performs:
//!
//! ```papyrus
//! While a > 0
//!     gv.SetValue(a * 33)
//! EndWhile
//! ```
//!
//! Writing to a `GlobalVariable`-like receiver that often is likely
//! unintended overhead compared to computing the final value locally and
//! writing it once after the loop.
//!
//! Never flagged when the loop's own body (not a further nested `While`'s
//! body, which is checked as its own separate loop) also calls
//! `Utility.Wait`, `RegisterForUpdate`, `RegisterForSingleUpdate`,
//! `RegisterForUpdateGameTime`, or `RegisterForSingleUpdateGameTime` (the
//! same family [`crate::short_wait_interval`] checks), since that's a
//! strong signal the write is deliberately paced (e.g. a progress value
//! updated once per tick) rather than happening in a tight, uncontrolled
//! loop.
//!
//! Like [`crate::repeated_getvalue`]/[`crate::global_variable_setvalue`], a
//! call's receiver can't generally be resolved back to a
//! `GlobalVariable`-typed script, so this matches by the `SetValue`/
//! `SetValueInt` method name alone (case-insensitively) rather than
//! requiring the receiver's declared type. Unrelated to
//! [`crate::global_variable_setvalue`], whose own heuristic instead checks
//! whether an `If`/`ElseIf`/`Else` chain already proves the write is a
//! no-op.

use papyrus_parser::ast::{Expr, IfBranch, Stmt};

use crate::short_wait_interval;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "setvalue-in-loop";


#[derive(Default)]
struct Collect {
    store: crate::visitor::Store,
}

impl crate::visitor::AstLint for Collect {
    fn store(&mut self) -> &mut crate::visitor::Store {
        &mut self.store
    }

    fn visit_function(
        &mut self,
        function: &papyrus_parser::ast::FunctionDecl,
        ctx: &mut crate::visitor::VisitCtx<'_>,
    ) {
        let mut diagnostics = Vec::new();
        check_body(&function.body, &mut diagnostics);
        let _ = ctx;
        self.store.extend(diagnostics);
    }
}

pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every `While` loop in `source` for a `SetValue`/`SetValueInt`
/// call that runs on every (or every reached) iteration, unless the loop
/// also calls a wait/update-registration function somewhere within it.
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



fn check_body(body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::While { body, .. } => {
                check_while(body, diagnostics);
                check_body(body, diagnostics);
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch { body, .. } in branches {
                    check_body(body, diagnostics);
                }
                check_body(else_body, diagnostics);
            }
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }
}

/// Checks a single `While` loop's own `body`, flagging every `SetValue`/
/// `SetValueInt` call found in it (see [`find_setvalue_calls`]) unless
/// `body` also calls a wait/update-registration function anywhere within it
/// (see [`contains_wait_call`]).
fn check_while(body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    if contains_wait_call(body) {
        return;
    }
    for write in find_setvalue_calls(body) {
        diagnostics.push(Diagnostic {
            line: write.line,
            column: 1,
            message: format!(
                "[warning] {}.{}(...) runs on every iteration of this loop, which is likely \
                 unintended overhead; consider computing the final value locally and writing it \
                 once after the loop, or pacing the loop with Utility.Wait/RegisterForUpdate if \
                 the repeated write is intentional",
                write.display, write.method
            ),
            rule: RULE,
        });
    }
}

/// A `<receiver>.SetValue()`/`SetValueInt(...)` call standing alone as its
/// own statement.
struct ValueWrite<'a> {
    display: String,
    method: &'a str,
    line: usize,
}

/// Finds every `<receiver>.SetValue()`/`SetValueInt(...)` call standing
/// alone as its own statement in `body`, recursing into a nested
/// `If`/`ElseIf`/`Else` within it, but not into a further nested `While`
/// (which [`check_body`] checks separately, on its own).
fn find_setvalue_calls(body: &[Stmt]) -> Vec<ValueWrite<'_>> {
    let mut out = Vec::new();
    collect_setvalue_calls(body, &mut out);
    out
}

fn collect_setvalue_calls<'a>(body: &'a [Stmt], out: &mut Vec<ValueWrite<'a>>) {
    for stmt in body {
        match stmt {
            Stmt::Expr { value, line } => {
                if let Expr::Call { callee, .. } = value {
                    if let Expr::Member { object, property } = callee.as_ref() {
                        if is_value_setter(property) {
                            out.push(ValueWrite {
                                display: receiver_display(object),
                                method: property.as_str(),
                                line: *line,
                            });
                        }
                    }
                }
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch { body, .. } in branches {
                    collect_setvalue_calls(body, out);
                }
                collect_setvalue_calls(else_body, out);
            }
            Stmt::While { .. } => {}
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Return { .. } => {}
        }
    }
}

fn is_value_setter(name: &str) -> bool {
    name.eq_ignore_ascii_case("SetValue") || name.eq_ignore_ascii_case("SetValueInt")
}

/// A human-readable rendering of a `SetValue`/`SetValueInt` call's receiver
/// for use in a diagnostic message. An identifier, `Self`, or a chain of
/// member accesses built from those renders as its own source text; anything
/// less direct (an index, a call, a cast, ...) still gets a flagged
/// diagnostic, just with a generic stand-in here rather than guessed-at text.
fn receiver_display(expr: &Expr) -> String {
    match expr {
        Expr::Identifier(name) => name.clone(),
        Expr::Self_ => "Self".to_string(),
        Expr::Member { object, property } => format!("{}.{}", receiver_display(object), property),
        _ => "This receiver".to_string(),
    }
}

/// Whether `body` calls a wait/update-registration function
/// ([`short_wait_interval::WAIT_FUNCTIONS`]) anywhere within it, including
/// inside a nested `If`/`ElseIf`/`Else`. A further nested `While`'s own
/// condition still counts (it's evaluated on every reached iteration of
/// `body`'s own loop), but not its body: pacing a nested loop internally
/// doesn't pace the outer one, which is checked independently (see
/// [`check_body`]).
fn contains_wait_call(body: &[Stmt]) -> bool {
    body.iter().any(stmt_contains_wait_call)
}

fn stmt_contains_wait_call(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::VarDecl(decl) => decl.value.as_ref().is_some_and(expr_contains_wait_call),
        Stmt::Assign { target, value, .. } => {
            expr_contains_wait_call(target) || expr_contains_wait_call(value)
        }
        Stmt::Expr { value, .. } => expr_contains_wait_call(value),
        Stmt::Return {
            value: Some(value), ..
        } => expr_contains_wait_call(value),
        Stmt::Return { value: None, .. } => false,
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            branches.iter().any(|branch| {
                expr_contains_wait_call(&branch.condition) || contains_wait_call(&branch.body)
            }) || contains_wait_call(else_body)
        }
        Stmt::While { condition, .. } => expr_contains_wait_call(condition),
    }
}

fn expr_contains_wait_call(expr: &Expr) -> bool {
    if let Expr::Call { callee, args, .. } = expr {
        if short_wait_interval::matching_function(callee).is_some() {
            return true;
        }
        return expr_contains_wait_call(callee) || args.iter().any(expr_contains_wait_call);
    }

    match expr {
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::Call { .. }
        | Expr::NewStruct { .. } => false,
        Expr::Binary { left, right, .. } => {
            expr_contains_wait_call(left) || expr_contains_wait_call(right)
        }
        Expr::Unary { operand, .. } => expr_contains_wait_call(operand),
        Expr::NamedArg { value, .. } => expr_contains_wait_call(value),
        Expr::Member { object, .. } => expr_contains_wait_call(object),
        Expr::Index { object, index } => {
            expr_contains_wait_call(object) || expr_contains_wait_call(index)
        }
        Expr::Cast { value, .. } => expr_contains_wait_call(value),
        Expr::NewArray { size, .. } => expr_contains_wait_call(size),
    }
}

#[cfg(test)]
#[path = "setvalue_in_loop_tests.rs"]
mod tests;
