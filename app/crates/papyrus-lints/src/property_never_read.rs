//! Flags script properties that are written in this script but never read.
//!
//! Compound assignments (`+=` and friends) count as writes, not reads.
//! A name on another object (`other.Score`) is not a use of this script's
//! `Score`. A `; @external` annotation on the declaration line opts out.
//! Scripts that do not parse are left unchecked.

use crate::property_usage;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;
use papyrus_parser::ast::Script;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "property-never-read";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, ctx: &mut VisitCtx<'_>) {
        let usage = property_usage::collect_usage(script);
        for property in property_usage::all_properties(script) {
            if property_usage::line_has_external(ctx.source, property.line) {
                continue;
            }
            let Some(entry) = usage.get(&property.name.to_ascii_lowercase()) else {
                continue;
            };
            if !entry.written || entry.read {
                continue;
            }
            self.store.emit(
                property.line,
                1,
                format!(
                    "[info] Property '{}' is written but never read",
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

/// Checks `source` for `Property` declarations that are assigned in this
/// script but whose value is never read. Flagged as an `[info]`.
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
#[path = "property_never_read_tests.rs"]
mod tests;
