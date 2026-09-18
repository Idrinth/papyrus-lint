//! Flags statements that appear after a `Return` within the same block,
//! since control flow can never reach them.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! the block structure of the function body; a script that doesn't parse
//! cleanly is left unchecked rather than guessed at.

use papyrus_parser::ast::{FunctionDecl, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unreachable-statement";

/// Checks `source` for statements that follow a `Return` in the same
/// block (a function/event body, an `If`/`ElseIf`/`Else` branch, or a
/// `While` body). Flagged as a `[warning]`.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config, external);

    let Some(script) = ast else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        check_body(&function.body, &mut diagnostics);
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

fn check_body(body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    let mut returned = false;
    for stmt in body {
        if returned {
            diagnostics.push(Diagnostic {
                line: stmt_line(stmt),
                column: 1,
                message: "[warning] Unreachable statement: this can never execute because the \
                          block already returned above it"
                    .to_string(),
                rule: RULE,
            });
        }
        match stmt {
            Stmt::Return { .. } => returned = true,
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    check_body(&branch.body, diagnostics);
                }
                check_body(else_body, diagnostics);
            }
            Stmt::While { body, .. } => check_body(body, diagnostics),
            _ => {}
        }
    }
}

fn stmt_line(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::VarDecl(decl) => decl.line,
        Stmt::Assign { line, .. } => *line,
        Stmt::Expr { line, .. } => *line,
        Stmt::Return { line, .. } => *line,
        Stmt::If { line, .. } => *line,
        Stmt::While { line, .. } => *line,
    }
}

#[cfg(test)]
#[path = "unreachable_statement_tests.rs"]
mod tests;
