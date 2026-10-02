//! Flags a function or event declared only inside a named `State`, with no
//! empty-state declaration to dispatch to.
//!
//! Per the Creation Kit wiki's State Reference, every function implemented
//! in a state must also be implemented in the empty state of the current
//! script or a parent. Calling a state-only function while the script is
//! in the empty state (or any other state that does not declare it) throws
//! or fails silently. `state-function-signature` only compares signatures
//! when an empty-state declaration already exists; this rule requires one.
//!
//! Same-script empty-state declarations are checked first. Parent
//! empty-state declarations count when
//! [`ExternalSignatures::has_empty_state_function`] can resolve the
//! `Extends` chain. An unresolved parent is left unflagged rather than
//! guessed at, matching `state-function-signature`'s caveat.
//!
//! Private helpers that are never called from outside a state that
//! declares them are exempt: they cannot be reached from the empty state.
//! Remote events (`Type.OnSomething`) belong to another script and are
//! skipped.

use std::collections::{HashMap, HashSet};

use papyrus_parser::ast::{AccessLevel, Expr, FunctionDecl, Script};
use papyrus_parser::comment_annotations::parse_line_annotations;

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "state-missing-empty-fallback";

struct StateDecl {
    name: String,
    state: String,
    line: usize,
    is_event: bool,
    private_helper: bool,
}

#[derive(Default)]
struct Collect {
    store: Store,
    lines: Vec<String>,
    extends: Option<String>,
    empty: HashSet<String>,
    /// States that declare each function name (both lowercased).
    declared_in: HashMap<String, HashSet<String>>,
    state_decls: Vec<StateDecl>,
    /// Same-script calls, as `(callee, enclosing state)`. `None` is the
    /// empty state (or any non-function context, such as a property).
    calls: Vec<(String, Option<String>)>,
    enclosing_state: Option<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.lines = ctx.source.split('\n').map(str::to_string).collect();
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.extends = script.extends.clone();
        self.empty = script
            .functions
            .iter()
            .map(|function| function.name.to_ascii_lowercase())
            .collect();
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        self.enclosing_state = function
            .state
            .as_ref()
            .map(|state| state.to_ascii_lowercase());
        let Some(state) = function.state.as_deref() else {
            return;
        };
        let key = function.name.to_ascii_lowercase();
        self.declared_in
            .entry(key)
            .or_default()
            .insert(state.to_ascii_lowercase());
        if function.name.contains('.') {
            return;
        }
        self.state_decls.push(StateDecl {
            name: function.name.clone(),
            state: state.to_string(),
            line: function.line,
            is_event: function.is_event,
            private_helper: !function.is_event && is_private_helper(function, &self.lines),
        });
    }

    fn visit_expr(&mut self, expr: &Expr, _ctx: &mut VisitCtx<'_>) {
        let Expr::Call { callee, .. } = expr else {
            return;
        };
        let Some(name) = local_call_name(callee) else {
            return;
        };
        self.calls
            .push((name.to_ascii_lowercase(), self.enclosing_state.clone()));
    }

    fn finish(&mut self, ctx: &mut VisitCtx<'_>) {
        for decl in &self.state_decls {
            let key = decl.name.to_ascii_lowercase();
            if self.empty.contains(&key) {
                continue;
            }
            if decl.private_helper && !called_outside(&self.calls, &self.declared_in, &key) {
                continue;
            }
            if let Some(parent) = self.extends.as_deref() {
                match ctx.external.has_empty_state_function(parent, &decl.name) {
                    Some(true) | None => continue,
                    Some(false) => {}
                }
            }
            let kind = if decl.is_event { "Event" } else { "Function" };
            self.store.emit(
                decl.line,
                1,
                format!(
                    "[warning] {kind} '{}' is declared only in state '{}' and has no empty-state \
                     fallback; calling it while the script is not in that state fails at runtime",
                    decl.name, decl.state
                ),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for state-only functions and events with no empty-state
/// fallback on this script or a resolvable parent.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

fn called_outside(
    calls: &[(String, Option<String>)],
    declared_in: &HashMap<String, HashSet<String>>,
    name: &str,
) -> bool {
    let states = declared_in.get(name);
    calls.iter().any(|(callee, state)| {
        callee == name
            && match state {
                None => true,
                Some(state) => !states.is_some_and(|states| states.contains(state)),
            }
    })
}

fn is_private_helper(function: &FunctionDecl, lines: &[String]) -> bool {
    function.access_level == AccessLevel::Private || has_private_annotation(function.line, lines)
}

fn has_private_annotation(function_line: usize, lines: &[String]) -> bool {
    if function_line == 0 {
        return false;
    }
    let header_index = function_line - 1;
    if lines
        .get(header_index)
        .is_some_and(|line| line_marks_private(line))
    {
        return true;
    }
    let mut index = header_index;
    while index > 0 {
        index -= 1;
        let Some(line) = lines.get(index) else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        if !is_line_comment(line) {
            break;
        }
        if line_marks_private(line) {
            return true;
        }
    }
    false
}

fn is_line_comment(line: &str) -> bool {
    line.trim_start().starts_with(';')
}

fn line_marks_private(line: &str) -> bool {
    parse_line_annotations(line)
        .iter()
        .any(|annotation| annotation.name.eq_ignore_ascii_case("private"))
}

fn local_call_name(callee: &Expr) -> Option<&str> {
    match callee {
        Expr::Identifier(name) => Some(name),
        Expr::Member { object, property } if matches!(object.as_ref(), Expr::Self_) => {
            Some(property)
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "state_missing_empty_fallback_tests.rs"]
mod tests;
