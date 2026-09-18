//! Flags a function/event parameter that gets assigned a new value inside
//! its own body, since reusing the parameter's name for a different value
//! shadows what the caller passed in and can confuse a reader who expects
//! it to still reflect the original argument at any later point in the
//! function.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to tell a parameter's own name apart from an unrelated local or
//! property with the same name; a script that doesn't parse cleanly is
//! left unchecked rather than guessed at.

use papyrus_parser::ast::{Expr, FunctionDecl, Script, Stmt};

use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "parameter-reassignment";

/// Checks `source` for a function/event parameter reassigned somewhere in
/// its own body. Flagged as a `[warning]`.
///
/// A reassignment inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers,
/// is never flagged: it's generated boilerplate the user can't edit.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::argument_types::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (tokens, config, external);

    let Some(script) = ast else {
        return Vec::new();
    };
    let protected = fragment_code::protected_lines(source);

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        if function.params.is_empty() {
            continue;
        }
        for assign in collect_assigns(&function.body) {
            let Stmt::Assign { target, line, .. } = assign else {
                continue;
            };
            let Expr::Identifier(name) = target else {
                continue;
            };
            if protected.get(*line).copied().unwrap_or(false) {
                continue;
            }
            let Some(param) = function
                .params
                .iter()
                .find(|param| param.name.eq_ignore_ascii_case(name))
            else {
                continue;
            };

            diagnostics.push(Diagnostic {
                line: *line,
                column: 1,
                message: format!(
                    "[warning] Parameter '{}' is reassigned inside its function; consider using a local variable instead",
                    param.name
                ),
                rule: RULE,
            });
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
#[path = "parameter_reassignment_tests.rs"]
mod tests;
