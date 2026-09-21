//! Flags a call through Papyrus's static/global call syntax
//! (`ScriptName.Function(...)`, e.g. `MyScript.DoThing()`) whose target
//! function isn't declared `Global` on that script — Papyrus only allows
//! the static syntax to reach a script's `Global` functions; calling an
//! ordinary instance function that way fails to compile, since it needs an
//! actual object reference (or `Self`) to run against.
//!
//! Like [`crate::unresolved_script`], only a call whose object is a bare
//! identifier not already known as a local variable, parameter, or
//! property is treated as a script reference at all — anything resolvable
//! locally is a normal instance call, left to the "Argument type check"
//! lint instead. Whether a resolved function is declared `Global` depends
//! on the project's own scripts, which this crate has no filesystem access
//! to on its own; a caller that can resolve it (e.g. the desktop app's
//! `FunctionTable`) does so by implementing
//! [`ExternalSignatures::is_global_function`] and calling [`check_with`]
//! instead of [`check`].

use papyrus_parser::ast::{Expr, FunctionDecl, Script};
use papyrus_parser::types::TypeEnv;

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "non-global-function-call";

#[derive(Default)]
struct Collect {
    store: Store,
    env: Option<TypeEnv>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.env = Some(TypeEnv::for_script(script));
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(env) = &mut self.env {
            env.enter_function(function);
        }
    }

    fn leave_function(&mut self, _function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(env) = &mut self.env {
            env.leave_function();
        }
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Some(env) = self.env.as_ref() else {
            return;
        };
        let Expr::Call {
            callee,
            line,
            col,
            ..
        } = expr
        else {
            return;
        };
        let Expr::Member { object, property } = &**callee else {
            return;
        };
        let Expr::Identifier(name) = &**object else {
            return;
        };
        if env.lookup(name).is_some() {
            return;
        }
        if ctx.external.is_global_function(name, property) == Some(false) {
            self.store.push(not_global(*line, *col, name, property));
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for calls through a script name whose target function
/// isn't declared `Global`. Since this crate has no filesystem access on
/// its own, no such call can ever be confirmed this way; see
/// [`check_with`] to actually resolve function signatures.
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

/// Like [`check`], but resolves each call's target function through
/// `external`, flagging one that resolves but isn't declared `Global`.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check_with<E: ExternalSignatures + ?Sized>(
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    crate::visitor::run(
        visitor(),
        "",
        ast,
        None,
        &crate::config::Config::default(),
        external,
    )
}

fn not_global(line: usize, col: usize, type_name: &str, function_name: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!(
            "[error] '{function_name}' is not declared Global on '{type_name}', so it can't be called as '{type_name}.{function_name}()' without an instance"
        ),
        rule: RULE,
    }
}

#[cfg(test)]
#[path = "non_global_function_call_tests.rs"]
mod tests;
