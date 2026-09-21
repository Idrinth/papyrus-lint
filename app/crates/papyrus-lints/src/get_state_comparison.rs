//! Flags a `GetState() == "Name"`/`GetState() != "Name"` comparison (bare
//! `GetState()` or `self.GetState()`) whose named state can't be found,
//! since a typo'd or renamed state name still compiles fine but the
//! comparison then silently, permanently evaluates the opposite of what was
//! intended — a check against a state that can never be the current one is
//! always `false`, and its negation always `true` — instead of raising an
//! error.
//!
//! Like [`crate::goto_state`], a target not declared on this script isn't
//! flagged when this script `Extends` another and the target can't be ruled
//! out there either (see [`check_with`]) — it may only be declared on a
//! script further up that (unresolved) `Extends` chain, a legitimate way
//! for this script to compare against a state a not-yet-written child
//! implements. The empty string (`GetState() == ""`, checking whether the
//! script is currently in the empty state) is always valid and never
//! flagged.

use papyrus_parser::ast::{BinaryOp, Expr, Literal, Script};

use crate::external_signatures::ExternalSignatures;
use crate::state_reference::StateReferences;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "get-state-comparison";

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
        let Expr::Binary { left, op, right } = expr else {
            return;
        };
        if !matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
            return;
        }
        check_comparison(
            left,
            right,
            &self.states,
            ctx.external,
            &mut self.store,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for `GetState()` comparisons against a state that can't
/// be found on the script itself. A script that `Extends` another is left
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
pub fn check_with<E: ExternalSignatures>(ast: Option<&Script>, external: &mut E) -> Vec<Diagnostic> {
    crate::visitor::run(
        visitor(),
        "",
        ast,
        None,
        &crate::config::Config::default(),
        external,
    )
}

/// Flags `left op right` when exactly one side is a bare/`self`-qualified
/// `GetState()` call (with no arguments) and the other is a string literal
/// naming a state that can't be found.
fn check_comparison<E: ExternalSignatures + ?Sized>(
    left: &Expr,
    right: &Expr,
    states: &StateReferences,
    external: &mut E,
    store: &mut Store,
) {
    let target = get_state_call(left)
        .zip(string_literal(right))
        .or_else(|| get_state_call(right).zip(string_literal(left)));

    if let Some(((line, col), name)) = target {
        if states.is_missing(name, external) {
            store.push(missing(line, col, name));
        }
    }
}

fn string_literal(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Literal(Literal::String(name)) => Some(name.as_str()),
        _ => None,
    }
}

/// Returns the line/column of `expr` if it's a bare/`self`-qualified
/// `GetState()` call taking no arguments.
fn get_state_call(expr: &Expr) -> Option<(usize, usize)> {
    match expr {
        Expr::Call {
            callee,
            args,
            line,
            col,
        } if args.is_empty() && is_get_state_callee(callee) => Some((*line, *col)),
        _ => None,
    }
}

/// Whether `callee` is a bare `GetState(...)` call, or one explicitly
/// qualified with `self.GetState(...)`. `GetState` always reports the
/// state of the script it's called from, so no other qualifier is
/// recognized.
fn is_get_state_callee(callee: &Expr) -> bool {
    match callee {
        Expr::Identifier(name) => name.eq_ignore_ascii_case("GetState"),
        Expr::Member { object, property } => {
            matches!(**object, Expr::Self_) && property.eq_ignore_ascii_case("GetState")
        }
        _ => false,
    }
}

fn missing(line: usize, col: usize, name: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!(
            "[error] GetState() compared against state '{name}', which could not be found"
        ),
        rule: RULE,
    }
}

#[cfg(test)]
#[path = "get_state_comparison_tests.rs"]
mod tests;
