//! Flags an unresolved parent script, declared type, or call through
//! Papyrus's static/global call syntax
//! (`ScriptName.Function(...)`, e.g. `Utility.Wait(1.0)` or
//! `MyMissingScript.DoThing()`) whose target script can't be located,
//! since Papyrus resolves that name against a script file at compile time
//! and a call through a script that doesn't exist can never compile.
//!
//! Only a call whose object is a bare identifier not already known as a
//! local variable, parameter, or property (i.e. definitely not an
//! instance the script already has a handle to) is treated as a script
//! reference at all — anything resolvable locally is left to the
//! "Argument type check"/"Return type check" lints instead. Whether such a
//! name, an `Extends` parent, or a type annotation can be located depends
//! on the project's own scripts and built-in native type data,
//! neither of which this crate has access to on its own; a caller that can
//! resolve them (e.g. the desktop app's `FunctionTable`) does so by
//! implementing [`ExternalSignatures::script_exists`] and
//! [`ExternalSignatures::type_exists`] and calling
//! [`check_with`] instead of [`check`].

use papyrus_parser::ast::{Expr, FunctionDecl, Script, TypeName};
use papyrus_parser::types::TypeEnv;

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unresolved-script";

#[derive(Default)]
struct Collect {
    store: Store,
    env: Option<TypeEnv>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, ctx: &mut VisitCtx<'_>) {
        self.env = Some(TypeEnv::for_script(script));
        if let Some(parent) = &script.extends {
            if !ctx.external.type_exists(parent) {
                self.store.push(missing_type(script.line, 1, parent, "Parent script"));
            }
        }
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

    fn visit_type_name(&mut self, type_name: &TypeName, ctx: &mut VisitCtx<'_>) {
        if !ctx.external.type_exists(&type_name.name) {
            self.store
                .push(missing_type(ctx.line, 1, &type_name.name, "Type"));
        }
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        match expr {
            Expr::Call {
                callee,
                line,
                col,
                ..
            } => {
                let Expr::Member { object, .. } = &**callee else {
                    return;
                };
                let Expr::Identifier(name) = &**object else {
                    return;
                };
                let Some(env) = self.env.as_ref() else {
                    return;
                };
                if env.lookup(name).is_none() && !ctx.external.script_exists(name) {
                    self.store.push(missing(*line, *col, name));
                }
            }
            Expr::Cast { type_name, .. } | Expr::Is { type_name, .. }
                if !ctx.external.type_exists(type_name) =>
            {
                self.store
                    .push(missing_type(ctx.line, 1, type_name, "Type"));
            }
            _ => {}
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for calls through a script name that can't be
/// resolved. Since this crate has no filesystem access on its own, no
/// script can ever be confirmed missing this way; see [`check_with`] to
/// actually resolve script names.
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

/// Like [`check`], but resolves each call's target script through
/// `external`, flagging one that can't be located.
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

fn missing_type(line: usize, col: usize, name: &str, kind: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!("[warning] {kind} '{name}' could not be located"),
        rule: RULE,
    }
}

fn missing(line: usize, col: usize, name: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!("[warning] Script '{name}' could not be located"),
        rule: RULE,
    }
}

#[cfg(test)]
#[path = "unresolved_script_tests.rs"]
mod tests;
