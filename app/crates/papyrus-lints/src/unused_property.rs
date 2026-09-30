//! Flags script properties that are declared but never referenced.
//!
//! This works from the parsed AST: a property name on another object
//! (`akRef.Foo`) is not a use of this script's `Foo`. Scripts that do
//! not parse are left unchecked, like other correctness rules.

use crate::property_usage;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;
use papyrus_parser::ast::Script;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-property";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        let usage = property_usage::collect_usage(script);
        for property in property_usage::all_properties(script) {
            let lower = property.name.to_ascii_lowercase();
            let used = usage
                .get(&lower)
                .is_some_and(|entry| entry.read || entry.written);
            if used {
                continue;
            }
            self.store.emit(
                property.line,
                1,
                format!(
                    "[warning] Property '{}' is declared but never used",
                    property.name
                ),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for `Property` declarations whose name is never used
/// anywhere else in the script. Flagged as a `[warning]`.
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
#[path = "unused_property_tests.rs"]
mod tests;
