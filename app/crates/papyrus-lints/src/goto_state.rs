//! Flags a `GoToState("Name")` call whose target state can't be found,
//! since a typo'd or renamed state name compiles fine but silently never
//! takes effect: the engine just falls back through the state resolution
//! algorithm instead of raising an error (see
//! <https://ck.uesp.net/wiki/State_Reference>).
//!
//! Per that same reference, a `GoToState` target does *not* have to exist
//! on the script it's called from — it may only be declared on a script
//! that extends this one, which is a legitimate way to forward-declare a
//! state for an as-yet-unwritten child script to implement. To keep that
//! pattern from being flagged, a target not declared on this script is
//! only reported when this script has no `Extends` at all (nothing else
//! could ever define it) or, once resolved through `external`'s knowledge
//! of the project (see [`check_with`]), when it isn't declared anywhere in
//! this script's own ancestry either. The empty string (`GoToState("")`,
//! switching back to the empty state) is always valid and never flagged.

use papyrus_parser::ast::{Expr, Literal, Script};

use crate::external_signatures::ExternalSignatures;
use crate::state_reference::StateReferences;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "goto-state";

#[derive(Default)]
struct Collect {
    store: Store,
    states: StateReferences,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.states = StateReferences::collect(script);
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Expr::Call {
            callee,
            args,
            line,
            col,
        } = expr
        else {
            return;
        };
        if !is_goto_state_callee(callee) {
            return;
        }
        let [Expr::Literal(Literal::String(name))] = args.as_slice() else {
            return;
        };
        if self.states.is_missing(name, ctx.external) {
            self.store.push(missing(*line, *col, name));
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for `GoToState` calls whose target state can't be found
/// on the script itself. A script that `Extends` another is left
/// unchecked when the target isn't declared locally, since it may be
/// declared further up that (unresolved) ancestry; see [`check_with`] to
/// resolve that too.
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

/// Like [`check`], but resolves a target not declared on the script itself
/// through `external`'s knowledge of the script's `Extends` ancestry,
/// flagging a target that can't be found there either.
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

/// Whether `callee` is a bare `GoToState(...)` call, or one explicitly
/// qualified with `self.GoToState(...)`. `GoToState` always acts on the
//! script it's called from, so no other qualifier is recognized.
fn is_goto_state_callee(callee: &Expr) -> bool {
    match callee {
        Expr::Identifier(name) => name.eq_ignore_ascii_case("GoToState"),
        Expr::Member { object, property } => {
            matches!(**object, Expr::Self_) && property.eq_ignore_ascii_case("GoToState")
        }
        _ => false,
    }
}

fn missing(line: usize, col: usize, name: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!("[warning] GoToState references state '{name}', which could not be found"),
        rule: RULE,
    }
}

#[cfg(test)]
#[path = "goto_state_tests.rs"]
mod tests;
