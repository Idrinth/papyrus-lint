//! Flags event handlers whose names are not declared by any ancestor.
//!
//! Papyrus accepts arbitrary `Event` declarations, but an event handler only
//! receives an event exposed by the type it extends. Project-aware callers
//! resolve that inheritance chain through [`crate::ExternalSignatures`]. When
//! no resolver is available, scripts are left alone rather than producing a
//! false positive.

use papyrus_parser::ast::{FunctionDecl, Script};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

pub const RULE: &str = "undefined-event";

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
        if !function.is_event {
            return;
        }

        let Some(parent) = self.extends.as_deref() else {
            return;
        };
        if !matches!(ctx.external.has_event(parent, &function.name), Some(false)) {
            return;
        }

        self.store.emit(
            function.line,
            1,
            format!(
                "[warning] Event '{}' is not defined by any ancestor",
                function.name
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

#[allow(dead_code)]
pub fn check(
    source: &str,
    ast: Option<&Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

#[cfg(test)]
#[path = "undefined_event_tests.rs"]
mod tests;
