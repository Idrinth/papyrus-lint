//! Flags a plain `=` assignment whose right-hand side is the exact same
//! reference as its own target (e.g. `a = a`, `Self.Foo = Self.Foo`,
//! `akRef.Foo = akRef.Foo`), since that assignment can never change the
//! value it reads and is almost always a copy-paste mistake or leftover
//! from a refactor.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to compare the target and value expressions structurally; a script
//! that doesn't parse cleanly is left unchecked rather than guessed at.
//!
//! Only a bare identifier or a chain of member accesses rooted at one (or
//! at `Self`) is ever compared this way — a call, an index, or any other
//! expression shape never counts as a self-assignment, since re-evaluating
//! it on both sides of the same line isn't guaranteed to read the same
//! value twice (or may have side effects of its own). A compound
//! assignment (`+=`, `-=`, ...) is never flagged either, since unlike
//! plain `=` it does change the target's value.

use papyrus_parser::ast::{AssignOp, Expr, Stmt};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "self-assignment";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_stmt(&mut self, stmt: &Stmt, ctx: &mut VisitCtx<'_>) {
        let Stmt::Assign {
            target,
            op: AssignOp::Assign,
            value,
            ..
        } = stmt
        else {
            return;
        };
        let (Some(target_key), Some(value_key)) = (reference_key(target), reference_key(value))
        else {
            return;
        };
        if target_key != value_key {
            return;
        }
        self.store.emit(
            ctx.line,
            1,
            "[warning] This assigns a value to itself, which has no effect",
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for a plain `=` assignment whose target and value are the
/// exact same simple reference. Flagged as a `[warning]`.
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

/// Deletes every self-assignment statement [`check`] would flag, removing
/// the whole source line (including its line ending) the same way
/// [`crate::unused_import::repair_with`] removes an unused `Import`.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);
    let Ok(script) = papyrus_parser::parse(source) else {
        return source.to_string();
    };
    let lines_to_remove: std::collections::HashSet<usize> = collect_self_assignment_lines(&script);
    if lines_to_remove.is_empty() {
        return source.to_string();
    }

    let mut result = String::with_capacity(source.len());
    let mut rest = source;
    let mut line_number = 1usize;
    while !rest.is_empty() {
        let (line_and_ending, remainder) = match rest.find('\n') {
            Some(index) => (&rest[..=index], &rest[index + 1..]),
            None => (rest, ""),
        };
        if !lines_to_remove.contains(&line_number) {
            result.push_str(line_and_ending);
        }
        rest = remainder;
        line_number += 1;
    }
    result
}

fn collect_self_assignment_lines(script: &papyrus_parser::ast::Script) -> std::collections::HashSet<usize> {
    let mut lines = std::collections::HashSet::new();
    for function in script
        .functions
        .iter()
        .chain(script.states.iter().flat_map(|state| state.functions.iter()))
    {
        collect_stmt_lines(&function.body, &mut lines);
    }
    lines
}

fn collect_stmt_lines(body: &[papyrus_parser::ast::Stmt], lines: &mut std::collections::HashSet<usize>) {
    use papyrus_parser::ast::{AssignOp, Stmt};
    for stmt in body {
        match stmt {
            Stmt::Assign {
                target,
                op: AssignOp::Assign,
                value,
                line,
            } => {
                if reference_key(target).is_some_and(|target_key| {
                    reference_key(value).is_some_and(|value_key| target_key == value_key)
                }) {
                    lines.insert(*line);
                }
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    collect_stmt_lines(&branch.body, lines);
                }
                collect_stmt_lines(else_body, lines);
            }
            Stmt::While { body, .. } => collect_stmt_lines(body, lines),
            Stmt::VarDecl(_) | Stmt::Expr { .. } | Stmt::Return { .. } | Stmt::Assign { .. } => {}
        }
    }
}

/// A canonical, case-insensitive key identifying a "simple reference"
/// expression (a bare identifier, `Self`, or a chain of member accesses
/// rooted at either), or `None` for any other expression shape (a call, an
/// index, a literal, ...), which is never compared for self-assignment.
fn reference_key(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(name) => Some(name.to_ascii_lowercase()),
        Expr::Self_ => Some("self".to_string()),
        Expr::Member { object, property } => Some(format!(
            "{}.{}",
            reference_key(object)?,
            property.to_ascii_lowercase()
        )),
        _ => None,
    }
}

#[cfg(test)]
#[path = "self_assignment_tests.rs"]
mod tests;
