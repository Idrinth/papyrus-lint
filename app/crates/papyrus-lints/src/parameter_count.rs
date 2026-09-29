//! Flags author-defined functions whose parameter list is longer than a
//! configured maximum.
//!
//! Engine event signatures are fixed by the runtime, so events are left
//! alone. Optional / defaulted parameters still count toward the total.

use papyrus_parser::ast::FunctionDecl;

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "parameter-count";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        if function.is_event {
            return;
        }
        let max = ctx.config.parameter_count_max;
        let count = function.params.len();
        if count <= max {
            return;
        }

        self.store.emit(
            ctx.line,
            1,
            format!(
                "[warning] Function '{}' has {} parameters (maximum: {})",
                function.name, count, max
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for author-defined functions whose parameter count
/// exceeds `parameter_count_max`. Events are never flagged.
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

#[cfg(test)]
#[path = "parameter_count_tests.rs"]
mod tests;
