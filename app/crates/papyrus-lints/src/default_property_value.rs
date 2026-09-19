//! Flags `Bool`/`Int`/`Float`/`String` `Auto`/`AutoReadOnly` properties
//! declared with no explicit default value (e.g. `Int Property Count
//! Auto` rather than `Int Property Count = 0 Auto`), since a property left
//! without one silently falls back to Papyrus's own implicit per-type
//! default (`False`, `0`, `0.0`, or `""`) instead of a value the author
//! actually chose.
//!
//! Disabled by default (see
//! [`crate::config::Rules::default_property_value`]): many existing
//! scripts already rely on Papyrus's implicit defaults for some or all of
//! their properties and don't need every one of them spelling out a value
//! explicitly, so a project has to opt in.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to reliably tell a property declaration (and its type) apart from other
//! identifiers; a script that doesn't parse cleanly is left unchecked
//! rather than guessed at. Object-typed properties, array-typed
//! properties, and full (non-`Auto`/`AutoReadOnly`) properties are never
//! flagged: only a `Bool`, `Int`, `Float`, or `String` scalar property
//! accepts a simple literal default at all.

use papyrus_parser::ast::PropertyDecl;

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "default-property-value";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_property(&mut self, property: &PropertyDecl, _ctx: &mut VisitCtx<'_>) {
        if !(property.is_auto || property.is_auto_read_only) || property.value.is_some() {
            return;
        }
        let Some(literal) = default_literal_for(property) else {
            return;
        };
        self.store.emit(
            property.line,
            1,
            format!(
                "[warning] {} Property '{}' has no explicit default value; consider '= {}'",
                property.type_name.name, property.name, literal
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for `Bool`/`Int`/`Float`/`String` `Auto`/`AutoReadOnly`
/// properties with no explicit default value, flagged as a `[warning]`.
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

/// The literal Papyrus would otherwise use as `property`'s implicit
/// default, or `None` when `property` isn't a scalar `Bool`/`Int`/`Float`/
/// `String` property (an array, or any other type) this lint doesn't cover.
fn default_literal_for(property: &PropertyDecl) -> Option<&'static str> {
    if property.type_name.is_array {
        return None;
    }
    let name = &property.type_name.name;
    if name.eq_ignore_ascii_case("Bool") {
        Some("False")
    } else if name.eq_ignore_ascii_case("Int") {
        Some("0")
    } else if name.eq_ignore_ascii_case("Float") {
        Some("0.0")
    } else if name.eq_ignore_ascii_case("String") {
        Some("\"\"")
    } else {
        None
    }
}

#[cfg(test)]
#[path = "default_property_value_tests.rs"]
mod tests;
