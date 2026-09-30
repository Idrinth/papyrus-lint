//! Flags a non-`Auto` property whose backing value is never written.
//!
//! Writes inside the property's Get/Set accessors do not count; a write
//! is an assignment to the property name or its backing field outside
//! those accessors, including a script-level variable initializer.
//! Disabled by default. Suppress a single property with `; @external`.
//! Scripts that do not parse are left unchecked.

use crate::property_usage;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;
use papyrus_parser::ast::Script;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "property-never-assigned";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, ctx: &mut VisitCtx<'_>) {
        for property in property_usage::all_properties(script) {
            if property.is_auto || property.is_auto_read_only {
                continue;
            }
            if property_usage::line_has_external(ctx.source, property.line) {
                continue;
            }
            let backing = property_usage::backing_fields(property);
            if backing.is_empty() {
                continue;
            }
            if property_usage::written_outside_accessors(script, property, &backing) {
                continue;
            }
            self.store.emit(
                property.line,
                1,
                format!(
                    "[warning] Property '{}' is never assigned; readers stay on the default value",
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

/// Checks `source` for non-`Auto` properties whose backing field and
/// property name are never written outside the property block.
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
#[path = "property_never_assigned_tests.rs"]
mod tests;
