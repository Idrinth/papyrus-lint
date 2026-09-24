//! Flags named states that can never become active because they are neither
//! marked `Auto` nor targeted by a literal `GoToState` call in the script.

use std::collections::HashSet;

use papyrus_parser::ast::{Expr, Literal, StateDecl};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-state";

#[derive(Default)]
struct Collect {
    store: Store,
    states: Vec<StateDecl>,
    targets: HashSet<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_state(&mut self, state: &StateDecl, _ctx: &mut VisitCtx<'_>) {
        self.states.push(state.clone());
    }

    fn visit_expr(&mut self, expr: &Expr, _ctx: &mut VisitCtx<'_>) {
        let Expr::Call { callee, args, .. } = expr else {
            return;
        };
        if !crate::goto_state::is_goto_state_callee(callee) {
            return;
        }
        let [Expr::Literal(Literal::String(name))] = args.as_slice() else {
            return;
        };
        self.targets.insert(name.to_ascii_lowercase());
    }

    fn finish(&mut self, _ctx: &mut VisitCtx<'_>) {
        for state in &self.states {
            if state.is_auto || self.targets.contains(&state.name.to_ascii_lowercase()) {
                continue;
            }
            self.store.emit(
                state.line,
                1,
                format!(
                    "[warning] State '{}' is neither Auto nor targeted by a GoToState call",
                    state.name
                ),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for named states that are neither `Auto` nor targeted by
/// a literal bare or `self`-qualified `GoToState` call.
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
#[path = "unused_state_tests.rs"]
mod tests;
