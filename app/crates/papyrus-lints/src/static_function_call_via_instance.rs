//! Flags a call reaching a script's `Global` function through an actual
//! object reference (a local variable, parameter, property, cast, or array
//! element — e.g. `akOtherActor.MyGlobalHelper()`) rather than Papyrus's
//! static/global call syntax (`ScriptName.MyGlobalHelper()`). Papyrus
//! allows this — a `Global` function ignores whatever reference it's
//! called through — but it can read as a mistake: a reader expecting an
//! instance method (or the author themselves, if the call was copy-pasted
//! from one) may not realize the reference is never actually used.
//!
//! The mirror image of [`crate::non_global_function_call`], which instead
//! flags a *non*-`Global` function reached through the static syntax.
//! `Self`/`Parent` are deliberately left unflagged: a script calling its
//! own `Global` function through `Self` for symmetry with its other
//! `Self.Whatever()` calls is a reasonable, common style rather than a
//! likely mistake. Whether a resolved function is declared `Global`
//! depends on the project's own scripts, which this crate has no
//! filesystem access to on its own; a caller that can resolve it (e.g. the
//! desktop app's `FunctionTable`) does so by implementing
//! [`ExternalSignatures::is_global_function`] and calling [`check_with`]
//! instead of [`check`].

use papyrus_parser::ast::{Expr, FunctionDecl, Script};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "static-function-call-via-instance";

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
        if matches!(**object, Expr::Self_ | Expr::Parent) {
            return;
        }
        let Some(object_type) = infer_type(object, env) else {
            return;
        };
        if object_type.is_array {
            return;
        }
        if ctx.external.is_global_function(&object_type.name, property) == Some(true) {
            self.store.push(called_via_instance(
                *line,
                *col,
                &object_type.name,
                property,
            ));
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for calls reaching a `Global` function through an
/// object reference. Since this crate has no filesystem access on its own,
/// no such call can ever be confirmed this way; see [`check_with`] to
/// actually resolve function signatures.
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
/// `external`, flagging one that resolves and is declared `Global`.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check_with<E: ExternalSignatures>(ast: Option<&Script>, external: &mut E) -> Vec<Diagnostic> {
    crate::visitor::run(
        visitor(),
        "",
        ast,
        None,
        &crate::config::Config::default(),
        external,
    )
}

fn called_via_instance(
    line: usize,
    col: usize,
    type_name: &str,
    function_name: &str,
) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!(
            "[warning] '{function_name}' is declared Global on '{type_name}', so it doesn't need an object reference — calling it as '{type_name}.{function_name}()' avoids the possibly-confusing implication that the object matters, but calling it through an instance still works"
        ),
        rule: RULE,
    }
}

#[cfg(test)]
#[path = "static_function_call_via_instance_tests.rs"]
mod tests;
