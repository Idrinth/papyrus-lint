//! Flags local variables that shadow a `Property` or a plain script-level
//! variable (field) declared on the same script or on a parent script,
//! since reading the name inside the function then reads the local rather
//! than the property/field, which is a common source of confusion (and,
//! once the local goes out of scope conceptually, bugs).
//!
//! Like [`crate::argument_types`], this works from the parsed AST rather
//! than raw tokens, and reuses that module's
//! [`argument_types::ExternalSignatures`] trait so a caller that can
//! resolve other scripts (e.g. the desktop app's `FunctionTable`) can also
//! check shadowing against a parent script's properties, not just the
//! linted script's own. A script that doesn't parse cleanly is left
//! unchecked rather than guessed at.

use std::collections::HashSet;

use papyrus_parser::ast::{FunctionDecl, Script, Stmt, VariableDecl};

use crate::argument_types::ExternalSignatures;
use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "local-variable-shadowing";

/// Checks `source` for local variables that shadow a property or field
/// declared on the same script. Shadowing a property or field declared on
/// a parent script isn't checked this way, since resolving parent scripts
/// to files requires filesystem access this crate deliberately doesn't
/// have; see [`check_with`] for that. Flagged as a `[warning]`.
///
/// A declaration inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers, is
/// never flagged, since it's generated boilerplate the user can't edit.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::argument_types::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (tokens, config);
    check_with(source, ast, external)
}

/// Like [`check`], but also flags a local variable that shadows a property
/// or field declared on a parent script, resolved (including through
/// `Extends`) through `external`.
pub fn check_with<E: ExternalSignatures>(
    source: &str,
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };
    let protected = fragment_code::protected_lines(source);

    let own_properties: HashSet<String> = script
        .properties
        .iter()
        .map(|p| p.name.to_ascii_lowercase())
        .collect();
    let own_variables: HashSet<String> = script
        .variables
        .iter()
        .map(|v| v.name.to_ascii_lowercase())
        .collect();

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        for decl in collect_var_decls(&function.body) {
            if protected.get(decl.line).copied().unwrap_or(false) {
                continue;
            }
            diagnostics.extend(check_decl(
                decl,
                script,
                &own_properties,
                &own_variables,
                external,
            ));
        }
    }
    diagnostics
}

fn check_decl<E: ExternalSignatures>(
    decl: &VariableDecl,
    script: &Script,
    own_properties: &HashSet<String>,
    own_variables: &HashSet<String>,
    external: &mut E,
) -> Option<Diagnostic> {
    let name_lower = decl.name.to_ascii_lowercase();

    if own_properties.contains(&name_lower) {
        return Some(Diagnostic {
            line: decl.line,
            column: 1,
            message: format!(
                "[warning] Local variable '{}' shadows this script's own property '{}'",
                decl.name, decl.name
            ),
            rule: RULE,
        });
    }

    if own_variables.contains(&name_lower) {
        return Some(Diagnostic {
            line: decl.line,
            column: 1,
            message: format!(
                "[warning] Local variable '{}' shadows this script's own variable '{}'",
                decl.name, decl.name
            ),
            rule: RULE,
        });
    }

    let parent = script.extends.as_ref()?;
    if external.has_property(parent, &decl.name) {
        return Some(Diagnostic {
            line: decl.line,
            column: 1,
            message: format!(
                "[warning] Local variable '{}' shadows a property '{}' inherited from a parent script",
                decl.name, decl.name
            ),
            rule: RULE,
        });
    }

    if external.has_field(parent, &decl.name) {
        return Some(Diagnostic {
            line: decl.line,
            column: 1,
            message: format!(
                "[warning] Local variable '{}' shadows a variable '{}' inherited from a parent script",
                decl.name, decl.name
            ),
            rule: RULE,
        });
    }

    None
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

/// Finds every `VariableDecl` in `body`, including ones nested inside
/// `If`/`ElseIf`/`Else` branches and `While` bodies, since Papyrus locals
/// aren't block-scoped.
fn collect_var_decls(body: &[Stmt]) -> Vec<&VariableDecl> {
    let mut decls = Vec::new();
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => decls.push(decl),
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    decls.extend(collect_var_decls(&branch.body));
                }
                decls.extend(collect_var_decls(else_body));
            }
            Stmt::While { body, .. } => decls.extend(collect_var_decls(body)),
            _ => {}
        }
    }
    decls
}

#[cfg(test)]
#[path = "local_variable_shadowing_tests.rs"]
mod tests;
