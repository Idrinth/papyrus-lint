//! Flags a completely empty `Event … EndEvent` in the empty/default state.
//!
//! An empty top-level event handler is often a forgotten stub. Empty
//! handlers inside a named `State` are left alone: overriding an inherited
//! or engine event with an empty body is the usual way to mute it for that
//! state. The first mention of an event in an inheritance hierarchy (a
//! definition rather than an override) is also left alone — see
//! [`Collect::visit_function`].
//!
//! `empty-body` covers empty `While`/`If`/`Else` bodies only;
//! `unnecessary-function` never flags `Event`s. This works from the parsed
//! AST, so a script that doesn't parse cleanly is left unchecked rather
//! than guessed at.

use papyrus_parser::ast::{FunctionDecl, Script};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "empty-event-handler";

#[derive(Default)]
struct Collect {
    store: Store,
    extends: Option<String>,
    /// Scripts that declare any `Native` member (or are marked `Native`
    /// themselves) are engine API declarations; their empty events are
    /// definitions, not forgotten handlers.
    engine_handled: bool,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.extends = script.extends.clone();
        self.engine_handled = script.is_native
            || script.functions.iter().any(|function| function.is_native)
            || script
                .states
                .iter()
                .flat_map(|state| &state.functions)
                .any(|function| function.is_native);
    }

    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        if !function.is_event
            || function.is_native
            || !function.body.is_empty()
            || function.state.is_some()
            || self.engine_handled
            // Qualified events are FO4/Starfield remote handlers and belong
            // to the type before the dot, not to this script's empty state.
            || function.name.contains('.')
        {
            return;
        }

        // No Extends means this is introducing the event, not overriding one.
        let Some(parent) = self.extends.as_deref() else {
            return;
        };

        // A known miss on every ancestor is a definition (or belongs to
        // `undefined-event`), not a forgotten override. Incomplete
        // resolution (`None`) still flags: an extending script's empty
        // top-level handler is usually meant to override an engine event.
        if matches!(ctx.external.has_event(parent, &function.name), Some(false)) {
            return;
        }

        self.store.emit(
            function.line,
            1,
            format!(
                "[info] Event '{}' has an empty body; this looks like a forgotten stub \
                 rather than something intentional (empty handlers inside a named State \
                 intentionally mute an event and are not flagged)",
                function.name
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for completely empty top-level `Event` handlers.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
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
#[path = "empty_event_handler_tests.rs"]
mod tests;
