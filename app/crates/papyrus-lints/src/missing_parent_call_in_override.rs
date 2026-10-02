//! Flags an overridden `Event` that never calls `Parent.Event()`.
//!
//! Overriding a lifecycle event on a project script drops the parent's
//! setup unless the child calls `Parent.OnInit()` (or whichever event it
//! is). Every event is checked — not a fixed list of names. The nearest
//! ancestor declaration is the one `Parent` would run: an empty or
//! `Return`-only parent event is a noop, so children are not warned, even
//! when a grandparent further up still has a body. `Native` scripts and
//! `Native` events are engine stubs where `Parent` is empty or irrelevant.
//!
//! A `; @no-parent-call` annotation on the header, or the line above it,
//! marks an intentional replacement. `; @disable missing-parent-call-in-override`
//! still works. Events inside a `State` are not checked: that is Papyrus's
//! state override, not `Extends`.

use papyrus_parser::ast::{Expr, FunctionDecl, Script, Stmt};
use papyrus_parser::comment_annotations::parse_line_annotations;

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "missing-parent-call-in-override";

#[derive(Default)]
struct Collect {
    store: Store,
    extends: Option<String>,
    lines: Vec<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.lines = ctx.source.split('\n').map(str::to_string).collect();
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.extends = script.extends.clone();
    }

    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        if !function.is_event
            || function.is_native
            || function.state.is_some()
            || function.name.contains('.')
        {
            return;
        }
        let Some(extends) = &self.extends else {
            return;
        };
        if header_opts_out(&self.lines, function.line)
            || body_calls_parent_event(&function.body, &function.name)
        {
            return;
        }
        if ctx
            .external
            .parent_event_needs_call(extends, &function.name)
            != Some(true)
        {
            return;
        }
        self.store.emit(
            function.line,
            1,
            format!(
                "[warning] Event '{}' overrides an inherited event on '{}' or one of its \
                 ancestors without calling Parent.{}()",
                function.name, extends, function.name
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for overridden events that never call `Parent`.
/// Without a resolver this never flags anything; see
/// [`crate::external_signatures::ExternalSignatures::parent_event_needs_call`].
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

fn header_opts_out(lines: &[String], line: usize) -> bool {
    if line == 0 {
        return false;
    }
    let index = line - 1;
    line_opts_out(lines.get(index).map(String::as_str).unwrap_or(""))
        || index
            .checked_sub(1)
            .and_then(|previous| lines.get(previous))
            .is_some_and(|previous| line_opts_out(previous))
}

fn line_opts_out(line: &str) -> bool {
    parse_line_annotations(line)
        .iter()
        .any(|annotation| annotation.name.eq_ignore_ascii_case("no-parent-call"))
}

fn body_calls_parent_event(body: &[Stmt], event_name: &str) -> bool {
    body.iter()
        .any(|stmt| stmt_calls_parent_event(stmt, event_name))
}

fn stmt_calls_parent_event(stmt: &Stmt, event_name: &str) -> bool {
    match stmt {
        Stmt::VarDecl(variable) => variable
            .value
            .as_ref()
            .is_some_and(|value| expr_calls_parent_event(value, event_name)),
        Stmt::Assign { target, value, .. } => {
            expr_calls_parent_event(target, event_name) || expr_calls_parent_event(value, event_name)
        }
        Stmt::Expr { value, .. } => expr_calls_parent_event(value, event_name),
        Stmt::Return { value, .. } => value
            .as_ref()
            .is_some_and(|value| expr_calls_parent_event(value, event_name)),
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            branches.iter().any(|branch| {
                expr_calls_parent_event(&branch.condition, event_name)
                    || body_calls_parent_event(&branch.body, event_name)
            }) || body_calls_parent_event(else_body, event_name)
        }
        Stmt::While {
            condition, body, ..
        } => {
            expr_calls_parent_event(condition, event_name)
                || body_calls_parent_event(body, event_name)
        }
        Stmt::LockGuard {
            body, else_body, ..
        } => {
            body_calls_parent_event(body, event_name)
                || body_calls_parent_event(else_body, event_name)
        }
    }
}

fn expr_calls_parent_event(expr: &Expr, event_name: &str) -> bool {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent | Expr::NewStruct { .. } => {
            false
        }
        Expr::Binary { left, right, .. } => {
            expr_calls_parent_event(left, event_name) || expr_calls_parent_event(right, event_name)
        }
        Expr::Unary { operand, .. } => expr_calls_parent_event(operand, event_name),
        Expr::Call { callee, args, .. } => {
            is_parent_event_call(callee, event_name)
                || expr_calls_parent_event(callee, event_name)
                || args
                    .iter()
                    .any(|arg| expr_calls_parent_event(arg, event_name))
        }
        Expr::NamedArg { value, .. } => expr_calls_parent_event(value, event_name),
        Expr::Member { object, .. } => expr_calls_parent_event(object, event_name),
        Expr::Index { object, index } => {
            expr_calls_parent_event(object, event_name) || expr_calls_parent_event(index, event_name)
        }
        Expr::Cast { value, .. } | Expr::Is { value, .. } => expr_calls_parent_event(value, event_name),
        Expr::NewArray { size, .. } => expr_calls_parent_event(size, event_name),
    }
}

fn is_parent_event_call(callee: &Expr, event_name: &str) -> bool {
    let Expr::Member { object, property } = callee else {
        return false;
    };
    matches!(object.as_ref(), Expr::Parent) && property.eq_ignore_ascii_case(event_name)
}

#[cfg(test)]
#[path = "missing_parent_call_in_override_tests.rs"]
mod tests;
