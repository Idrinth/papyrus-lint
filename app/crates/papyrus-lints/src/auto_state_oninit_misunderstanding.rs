//! Flags `Event OnInit()` declared inside a non-`Auto` named `State`.
//!
//! Papyrus fires `OnInit` when the script attaches — before any later
//! `GoToState` — so a state-nested `OnInit` only runs if that state is the
//! auto/entry state. Nested under a non-`Auto` state it is effectively
//! dead. Prefer empty-state / `Auto`-state `OnInit`, and `OnBeginState` for
//! per-state entry work.
//!
//! Related but different: [`crate::multiple_auto_states`],
//! [`crate::function_override`], [`crate::state_function_signature`].

use std::collections::HashMap;

use papyrus_parser::ast::{FunctionDecl, Script};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "auto-state-oninit-misunderstanding";

#[derive(Default)]
struct Collect {
    store: Store,
    /// Lowercased state name → whether that state was declared `Auto`.
    auto_by_state: HashMap<String, bool>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        let mut auto_by_state = HashMap::new();
        for state in &script.states {
            auto_by_state
                .entry(state.name.to_ascii_lowercase())
                .or_insert(state.is_auto);
        }
        self.auto_by_state = auto_by_state;
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if !function.is_event || !function.name.eq_ignore_ascii_case("OnInit") {
            return;
        }
        let Some(state_name) = function.state.as_deref() else {
            return;
        };
        let is_auto = self
            .auto_by_state
            .get(&state_name.to_ascii_lowercase())
            .copied()
            .unwrap_or(false);
        if is_auto {
            return;
        }
        self.store.emit(
            function.line,
            1,
            format!(
                "[warning] Event OnInit() inside non-Auto state '{state_name}' never runs at \
                 attach time; put OnInit in the empty state or an Auto state, and use \
                 OnBeginState for state entry"
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for `OnInit` events nested under non-`Auto` named states.
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
#[path = "auto_state_oninit_misunderstanding_tests.rs"]
mod tests;
