//! Flags a `Property` whose declared type, followed through that script's
//! own `Property` declarations across the project, eventually leads back to
//! the script being linted.

use std::collections::HashSet;

use papyrus_parser::ast::{PropertyDecl, Script};

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

pub const RULE: &str = "circular-dependency";

#[derive(Default)]
struct Collect {
    store: Store,
    origin: String,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.origin = script.name.clone();
    }

    fn visit_property(&mut self, property: &PropertyDecl, ctx: &mut VisitCtx<'_>) {
        if self.origin.is_empty() {
            return;
        }
        if property.type_name.name.eq_ignore_ascii_case(&self.origin) {
            return;
        }
        let mut visited = HashSet::new();
        let Some(chain) = cycle_through(
            &property.type_name.name,
            &self.origin,
            ctx.external,
            &mut visited,
        ) else {
            return;
        };
        self.store.emit(
            property.line,
            1,
            format!(
                "[warning] Property '{}' creates a circular dependency: {} -> {}",
                property.name,
                self.origin,
                chain.join(" -> "),
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
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

pub fn check_with<E: ExternalSignatures + ?Sized>(
    ast: Option<&papyrus_parser::ast::Script>,
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

fn cycle_through<E: ExternalSignatures + ?Sized>(
    current_type: &str,
    origin: &str,
    external: &mut E,
    visited: &mut HashSet<String>,
) -> Option<Vec<String>> {
    if current_type.eq_ignore_ascii_case(origin) {
        return Some(vec![current_type.to_string()]);
    }
    if !visited.insert(current_type.to_ascii_lowercase()) {
        return None;
    }
    for next_type in external.property_types(current_type) {
        if let Some(mut chain) = cycle_through(&next_type, origin, external, visited) {
            chain.insert(0, current_type.to_string());
            return Some(chain);
        }
    }
    None
}

#[cfg(test)]
#[path = "circular_dependency_tests.rs"]
mod tests;
