//! Flags a function declared on this script that shares its name with a
//! function declared on the script's `Extends` chain, since the local
//! declaration silently replaces the inherited one whenever this script's
//! type is used. This is often intentional (e.g. overriding an `Event
//! OnInit()` handler is the normal way to specialize a parent script's
//! behavior), so it's flagged as an `[info]` rather than a `[warning]`: a
//! useful thing to be aware of, not a likely mistake.
//!
//! Unlike the other lints in this crate, this can never be answered from
//! `source` alone — the parent script's declared functions live in a
//! different file. Like [`crate::argument_types`] and
//! [`crate::return_types`], it reuses
//! [`crate::external_signatures::ExternalSignatures`] so a caller that can
//! resolve other scripts (e.g. the desktop app's `FunctionTable`) supplies
//! that; without one (see [`check`]), this never finds anything to flag.
//!
//! Only functions declared directly on the script are checked, not ones
//! declared inside a `State` block — overriding a base state's function
//! from a named state is Papyrus's separate state-based override
//! mechanism, not `Extends` inheritance.

use papyrus_parser::ast::{FunctionDecl, Script};

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "function-override";

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
        if ctx.external.lookup(extends, &function.name).is_none() {
            return;
        }
        let kind = if function.is_event { "Event" } else { "Function" };
        self.store.emit(
            function.line,
            1,
            format!(
                "[info] {kind} '{}' overrides an inherited {} declared on '{}' or one of its ancestors",
                function.name,
                kind.to_ascii_lowercase(),
                extends
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for functions that override an inherited one. Since
/// resolving the `Extends` chain always requires looking outside `source`,
/// this alone never finds anything to flag; see [`check_with`].
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

/// Like [`check`], but resolves the script's `Extends` chain through
/// `external`, flagging any function declared on `source` whose name is
/// also declared somewhere along that chain.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check_with<E: ExternalSignatures>(
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
#[path = "function_override_tests.rs"]
mod tests;
