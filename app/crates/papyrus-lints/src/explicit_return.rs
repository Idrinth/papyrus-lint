//! Flags a typed function/event whose control flow can reach the end of its
//! body without hitting a `Return` statement, since Papyrus then silently
//! returns that type's default value (`0`, `""`, `False`, or `None`)
//! instead of a value the author actually chose.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! the block structure of the function body; a script that doesn't parse
//! cleanly is left unchecked rather than guessed at. A function with no
//! declared return type isn't checked (falling off its end is the normal,
//! intended way for it to finish). A `While` loop is never assumed to
//! guarantee a `Return`, since it may run zero times; an `If` only
//! guarantees one when every branch (`If`/`ElseIf`, and an `Else`) does, so
//! an `If` with no `Else` never counts, matching the fact that its
//! condition might not match any branch at runtime. A native function
//! (no body to inspect) is never flagged.

use papyrus_parser::ast::{FunctionDecl, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "explicit-return";

/// Checks `source` for typed functions/events with a code path that falls
/// off the end of the body without an explicit `Return`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    all_functions(&script)
        .filter(|function| function.return_type.is_some())
        .filter(|function| !function.is_native)
        .filter(|function| !body_always_returns(&function.body))
        .map(|function| Diagnostic {
            line: function.line,
            column: 1,
            message: format!(
                "[error] Function '{}' does not return a value on every code path",
                function.name
            ),
            rule: RULE,
        })
        .collect()
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

/// Whether every path through `body` is guaranteed to execute a `Return`
/// before falling off the end of the block.
fn body_always_returns(body: &[Stmt]) -> bool {
    body.iter().any(stmt_always_returns)
}

fn stmt_always_returns(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return { .. } => true,
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            body_always_returns(else_body)
                && branches
                    .iter()
                    .all(|branch| body_always_returns(&branch.body))
        }
        Stmt::While { .. } | Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } => false,
    }
}

#[cfg(test)]
#[path = "explicit_return_tests.rs"]
mod tests;
