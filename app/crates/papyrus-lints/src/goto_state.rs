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

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Literal, Script, Stmt};

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
pub fn check_with<E: ExternalSignatures + ?Sized>(
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };

    let states = StateReferences::collect(script);

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        for stmt in &function.body {
            walk_stmt(stmt, &states, external, &mut diagnostics);
        }
    }
    diagnostics
}

/// Iterates every function declared directly on a script, plus every
/// function declared in each of its states.
fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

fn walk_stmt<E: ExternalSignatures + ?Sized>(
    stmt: &Stmt,
    states: &StateReferences,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr(value, states, external, diagnostics);
            }
        }
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, states, external, diagnostics);
            walk_expr(value, states, external, diagnostics);
        }
        Stmt::Expr { value, .. } => walk_expr(value, states, external, diagnostics),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr(value, states, external, diagnostics);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for IfBranch {
                condition, body, ..
            } in branches
            {
                walk_expr(condition, states, external, diagnostics);
                for stmt in body {
                    walk_stmt(stmt, states, external, diagnostics);
                }
            }
            for stmt in else_body {
                walk_stmt(stmt, states, external, diagnostics);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, states, external, diagnostics);
            for stmt in body {
                walk_stmt(stmt, states, external, diagnostics);
            }
        }
    }
}

fn walk_expr<E: ExternalSignatures + ?Sized>(
    expr: &Expr,
    states: &StateReferences,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Call {
            callee,
            args,
            line,
            col,
        } => {
            if is_goto_state_callee(callee) {
                if let [Expr::Literal(Literal::String(name))] = args.as_slice() {
                    if states.is_missing(name, external) {
                        diagnostics.push(missing(*line, *col, name));
                    }
                }
            }
            walk_expr(callee, states, external, diagnostics);
            for arg in args {
                walk_expr(arg, states, external, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, states, external, diagnostics);
            walk_expr(right, states, external, diagnostics);
        }
        Expr::Unary { operand, .. } => {
            walk_expr(operand, states, external, diagnostics)
        }
        Expr::Member { object, .. } => {
            walk_expr(object, states, external, diagnostics)
        }
        Expr::Index { object, index } => {
            walk_expr(object, states, external, diagnostics);
            walk_expr(index, states, external, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, states, external, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, states, external, diagnostics),
        Expr::NamedArg { value, .. } => {
            walk_expr(value, states, external, diagnostics)
        }
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Whether `callee` is a bare `GoToState(...)` call, or one explicitly
/// qualified with `self.GoToState(...)`. `GoToState` always acts on the
/// script it's called from, so no other qualifier is recognized.
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
