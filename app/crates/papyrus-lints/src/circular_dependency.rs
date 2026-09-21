//! Flags a `Property` whose declared type, followed through that script's
//! own `Property` declarations across the project, eventually leads back to
//! the script being linted — a circular dependency between two (or more)
//! scripts, e.g.:
//!
//! ```papyrus
//! ScriptName A
//!
//! B Property Little Auto
//! ```
//!
//! together with:
//!
//! ```papyrus
//! ScriptName B
//!
//! A Property Large Auto
//! ```
//!
//! Papyrus itself compiles this fine — a `Property` is just a reference, not
//! an `Extends` chain — but a cycle like this still makes the two (or more)
//! scripts hard to reason about or reuse independently, since neither can be
//! fully understood without the other. A property whose own declared type is
//! the script it's declared on (a direct self-reference, e.g. a linked-list
//! node holding a `Property` of its own type) is never flagged: that's a
//! single script depending on itself, not a dependency between two scripts,
//! and is a common, deliberate pattern rather than a design smell. Since
//! this crate has no filesystem access on its own, following a property's
//! type chain across other scripts needs a resolver that can (e.g. the
//! desktop app's `FunctionTable`); see [`check_with`].

use std::collections::HashSet;

use papyrus_parser::ast::{PropertyDecl, Script};

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
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

/// Checks `source` for a `Property` whose declared type, followed through
/// other scripts' own `Property` declarations, cycles back to this script.
/// Since this crate has no filesystem access on its own, no such chain can
/// ever be confirmed this way; see [`check_with`] to actually follow
/// property types across scripts.
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

/// Like [`check`], but follows each property's declared type through
/// `external`, flagging one whose chain of `Property` declarations across
/// other scripts leads back to this script.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check_with<E: ExternalSignatures>(
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

/// Depth-first search over the property-type graph starting at
/// `current_type`, looking for a path back to `origin` (matched
/// case-insensitively). `visited` remembers every type already fully
/// explored — whether or not it led back to `origin` — so a cycle among
/// *other* scripts that never reaches `origin` is only ever walked once
/// instead of looping forever. Returns the chain of script names from
/// `current_type` back to `origin` (inclusive of both ends) the first time
/// one is found.
fn cycle_through<E: ExternalSignatures>(
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
