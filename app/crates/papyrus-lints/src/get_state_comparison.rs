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

use std::collections::HashSet;

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt};

use crate::external_signatures::ExternalSignatures;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "get-state-comparison";

#[derive(Default)]
struct Collect {
    store: crate::visitor::Store,
}

impl crate::visitor::AstLint for Collect {
    fn store(&mut self) -> &mut crate::visitor::Store {
        &mut self.store
    }

    fn visit_script(
        &mut self,
        script: &papyrus_parser::ast::Script,
        ctx: &mut crate::visitor::VisitCtx<'_>,
    ) {
        self.store.extend(lint_issues(
            ctx.source,
            Some(script),
            ctx.tokens,
            ctx.config,
            ctx.external,
        ));
    }

    fn finish(&mut self, ctx: &mut crate::visitor::VisitCtx<'_>) {
        if ctx.ast.is_none() {
            self.store.extend(lint_issues(
                ctx.source,
                None,
                ctx.tokens,
                ctx.config,
                ctx.external,
            ));
        }
    }
}

pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::Ast(Box::new(Collect::default()))
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

fn lint_issues(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut dyn crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config);
    check_with(ast, external)
}

/// Like [`check`], but resolves a target not declared on the script itself
/// through `external`'s knowledge of the script's `Extends` ancestry,
/// flagging a target that can't be found there either.
pub fn check_with<E: ExternalSignatures + ?Sized>(
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };

    let local_states: HashSet<String> = script
        .states
        .iter()
        .map(|state| state.name.to_ascii_lowercase())
        .collect();

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        for stmt in &function.body {
            walk_stmt(stmt, script, &local_states, external, &mut diagnostics);
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
    script: &Script,
    local_states: &HashSet<String>,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr(value, script, local_states, external, diagnostics);
            }
        }
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, script, local_states, external, diagnostics);
            walk_expr(value, script, local_states, external, diagnostics);
        }
        Stmt::Expr { value, .. } => walk_expr(value, script, local_states, external, diagnostics),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr(value, script, local_states, external, diagnostics);
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
                walk_expr(condition, script, local_states, external, diagnostics);
                for stmt in body {
                    walk_stmt(stmt, script, local_states, external, diagnostics);
                }
            }
            for stmt in else_body {
                walk_stmt(stmt, script, local_states, external, diagnostics);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, script, local_states, external, diagnostics);
            for stmt in body {
                walk_stmt(stmt, script, local_states, external, diagnostics);
            }
        }
    }
}

fn walk_expr<E: ExternalSignatures + ?Sized>(
    expr: &Expr,
    script: &Script,
    local_states: &HashSet<String>,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Expr::Binary { left, op, right } = expr {
        if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
            check_comparison(left, right, script, local_states, external, diagnostics);
        }
        walk_expr(left, script, local_states, external, diagnostics);
        walk_expr(right, script, local_states, external, diagnostics);
        return;
    }

    match expr {
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, script, local_states, external, diagnostics);
            for arg in args {
                walk_expr(arg, script, local_states, external, diagnostics);
            }
        }
        Expr::Unary { operand, .. } => {
            walk_expr(operand, script, local_states, external, diagnostics)
        }
        Expr::Member { object, .. } => {
            walk_expr(object, script, local_states, external, diagnostics)
        }
        Expr::Index { object, index } => {
            walk_expr(object, script, local_states, external, diagnostics);
            walk_expr(index, script, local_states, external, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, script, local_states, external, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, script, local_states, external, diagnostics),
        Expr::NamedArg { value, .. } => {
            walk_expr(value, script, local_states, external, diagnostics)
        }
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::Binary { .. } => {}
    }
}

/// Flags `left op right` when exactly one side is a bare/`self`-qualified
/// `GetState()` call (with no arguments) and the other is a string literal
/// naming a state that can't be found.
fn check_comparison<E: ExternalSignatures + ?Sized>(
    left: &Expr,
    right: &Expr,
    script: &Script,
    local_states: &HashSet<String>,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let target = get_state_call(left)
        .zip(string_literal(right))
        .or_else(|| get_state_call(right).zip(string_literal(left)));

    if let Some(((line, col), name)) = target {
        if is_missing(name, script, local_states, external) {
            diagnostics.push(missing(line, col, name));
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

/// Whether `name` can't be resolved as a state this script could ever be
/// in: not the empty string, not declared locally, and — when this script
/// `Extends` another — not found in that ancestry either (per `external`;
/// see the module docs).
fn is_missing<E: ExternalSignatures + ?Sized>(
    name: &str,
    script: &Script,
    local_states: &HashSet<String>,
    external: &mut E,
) -> bool {
    if name.is_empty() || local_states.contains(&name.to_ascii_lowercase()) {
        return false;
    }
    match &script.extends {
        None => true,
        Some(parent) => !external.has_state(parent, name),
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
