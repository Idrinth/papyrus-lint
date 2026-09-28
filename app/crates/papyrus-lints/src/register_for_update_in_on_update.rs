//! Flags, as a `[warning]`, a call to the repeating `RegisterForUpdate` or
//! `RegisterForUpdateGameTime` from inside an `Event OnUpdate` or
//! `Event OnUpdateGameTime` body, since each firing then schedules another
//! repeating registration on top of the ones already active and the script's
//! timer count grows without bound.
//!
//! Skyrim's one-shot `RegisterForSingleUpdate` /
//! `RegisterForSingleUpdateGameTime` exist specifically for the common "do
//! work, then schedule the next tick" pattern and do not stack that way —
//! those forms are left alone. Matches by function name alone
//! (case-insensitively), unqualified or through any receiver, the same way
//! [`crate::short_wait_interval`] does.
//!
//! Scoped to Skyrim via the rule's `games` field in `shared/rules.json`
//! (Fallout 4 replaced this API with `StartTimer` / `OnTimer`).

use papyrus_parser::ast::{Expr, FunctionDecl};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "register-for-update-in-on-update";

const UPDATE_HANDLERS: &[&str] = &["OnUpdate", "OnUpdateGameTime"];
const PERIODIC_REGISTERS: &[&str] = &["RegisterForUpdate", "RegisterForUpdateGameTime"];

#[derive(Default)]
struct Collect {
    store: Store,
    /// Depth of nested `Event OnUpdate` / `OnUpdateGameTime` bodies currently
    /// being visited (a nested call into another event is rare, but depth
    /// keeps enter/leave balanced if it happens).
    in_update_handler: usize,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if function.is_event && is_update_handler(&function.name) {
            self.in_update_handler += 1;
        }
    }

    fn leave_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if function.is_event && is_update_handler(&function.name) {
            self.in_update_handler = self.in_update_handler.saturating_sub(1);
        }
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        if self.in_update_handler == 0 {
            return;
        }
        let Expr::Call { callee, .. } = expr else {
            return;
        };
        let Some(name) = periodic_register_name(callee) else {
            return;
        };
        self.store.emit(
            ctx.line,
            1,
            format!(
                "[warning] {name}(...) inside an OnUpdate/OnUpdateGameTime handler stacks \
                 another repeating timer on every tick; use RegisterForSingleUpdate (or \
                 RegisterForSingleUpdateGameTime) to schedule the next run instead"
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for a repeating `RegisterForUpdate*` call inside an
/// `OnUpdate*` event handler.
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

fn is_update_handler(name: &str) -> bool {
    UPDATE_HANDLERS
        .iter()
        .any(|handler| handler.eq_ignore_ascii_case(name))
}

fn periodic_register_name(callee: &Expr) -> Option<&'static str> {
    let name = match callee {
        Expr::Identifier(name) => name.as_str(),
        Expr::Member { property, .. } => property.as_str(),
        _ => return None,
    };
    PERIODIC_REGISTERS
        .iter()
        .copied()
        .find(|register| register.eq_ignore_ascii_case(name))
}

#[cfg(test)]
#[path = "register_for_update_in_on_update_tests.rs"]
mod tests;
