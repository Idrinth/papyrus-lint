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

use papyrus_parser::ast::{AssignOp, Expr, FunctionDecl, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "self-assignment";

/// Checks `source` for a plain `=` assignment whose target and value are the
/// exact same simple reference. Flagged as a `[warning]`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        for assign in collect_assigns(&function.body) {
            let Stmt::Assign {
                target,
                op: AssignOp::Assign,
                value,
                line,
            } = assign
            else {
                continue;
            };
            let (Some(target_key), Some(value_key)) = (reference_key(target), reference_key(value))
            else {
                continue;
            };
            if target_key != value_key {
                continue;
            }

            diagnostics.push(Diagnostic {
                line: *line,
                column: 1,
                message: "[warning] This assigns a value to itself, which has no effect"
                    .to_string(),
                rule: RULE,
            });
        }
    }
    diagnostics
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

/// Finds every `Assign` statement in `body`, including ones nested inside
/// `If`/`ElseIf`/`Else` branches and `While` bodies.
fn collect_assigns(body: &[Stmt]) -> Vec<&Stmt> {
    let mut assigns = Vec::new();
    for stmt in body {
        match stmt {
            Stmt::Assign { .. } => assigns.push(stmt),
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    assigns.extend(collect_assigns(&branch.body));
                }
                assigns.extend(collect_assigns(else_body));
            }
            Stmt::While { body, .. } => assigns.extend(collect_assigns(body)),
            _ => {}
        }
    }
    assigns
}

#[cfg(test)]
#[path = "self_assignment_tests.rs"]
mod tests;
