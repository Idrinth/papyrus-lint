//! Flags a call through Papyrus's static/global call syntax
//! (`ScriptName.Function(...)`) whose target function isn't declared `Global`.

use papyrus_parser::ast::{Expr, FunctionDecl, Script};
use papyrus_parser::types::TypeEnv;

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

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

#[allow(dead_code)]
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

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
