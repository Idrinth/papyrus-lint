//! Flags a script-level `Property` or variable (declared directly on the
//! script, outside any function) whose name matches (case-insensitively)
//! the name of the script it's declared in, since Papyrus doesn't allow a
//! declared identifier to collide with the script's own type name — such a
//! script fails to compile.
//!
//! Works from the parsed AST rather than raw tokens, since a script's own
//! declared name, properties, and variables are already tracked there. A
//! local variable declared inside a function isn't checked here — see
//! [`crate::local_variable_shadowing`] for shadowing concerns local to a
//! function body. A script that doesn't parse cleanly is left unchecked
//! rather than guessed at.

use papyrus_parser::ast::{PropertyDecl, VariableDecl};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "script-name-collision";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_property(&mut self, property: &PropertyDecl, ctx: &mut VisitCtx<'_>) {
        let Some(script) = ctx.ast else {
            return;
        };
        if !property.name.eq_ignore_ascii_case(&script.name) {
            return;
        }
        self.store.emit(
            property.line,
            1,
            format!(
                "[error] Property '{}' may not share its name with the script it's declared in ('{}')",
                property.name, script.name
            ),
            RULE,
        );
    }

    fn visit_variable(&mut self, variable: &VariableDecl, ctx: &mut VisitCtx<'_>) {
        let Some(script) = ctx.ast else {
            return;
        };
        // `visit_variable` also fires for function-local `VarDecl`s; this lint
        // only flags script-level declarations.
        if !script
            .variables
            .iter()
            .any(|declared| std::ptr::eq(declared, variable))
        {
            return;
        }
        if !variable.name.eq_ignore_ascii_case(&script.name) {
            return;
        }
        self.store.emit(
            variable.line,
            1,
            format!(
                "[error] Variable '{}' may not share its name with the script it's declared in ('{}')",
                variable.name, script.name
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for a script-level `Property` or variable declaration
/// whose name matches (case-insensitively) the enclosing script's own
/// declared name. Flagged as an `[error]`, since Papyrus rejects such a
/// script at compile time.
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
#[path = "script_name_collision_tests.rs"]
mod tests;
