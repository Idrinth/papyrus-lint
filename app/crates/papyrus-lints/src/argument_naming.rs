//! Flags a function declared on this script whose parameter names don't
//! match (case-insensitively) the corresponding parameter names of the
//! same-named function declared on the script's `Extends` chain.

use papyrus_parser::ast::{FunctionDecl, Script};

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

pub const RULE: &str = "argument-naming";

#[derive(Default)]
struct Collect {
    store: Store,
    extends: Option<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.extends = script.extends.clone();
    }

    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        if function.state.is_some() {
            return;
        }
        let Some(extends) = &self.extends else {
            return;
        };
        let Some(parent_params) = ctx.external.lookup(extends, &function.name) else {
            return;
        };

        for (index, (local, parent)) in function.params.iter().zip(&parent_params).enumerate() {
            if local.name.eq_ignore_ascii_case(&parent.name) {
                continue;
            }
            self.store.emit(
                function.line,
                1,
                format!(
                    "[warning] Parameter {} of '{}' is named '{}' but the inherited declaration on '{}' names it '{}'",
                    index + 1,
                    function.name,
                    local.name,
                    extends,
                    parent.name
                ),
                RULE,
            );
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

#[cfg(test)]
#[path = "argument_naming_tests.rs"]
mod tests;
