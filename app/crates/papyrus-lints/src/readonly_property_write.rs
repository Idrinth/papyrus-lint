//! Flags an assignment that writes to a script-level property declared
//! `AutoReadOnly` (e.g. `Float Property a = 0.1 AutoReadOnly`), since
//! Papyrus rejects such an assignment at compile time — an `AutoReadOnly`
//! property can only ever be set to its declared initial value.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to tell an `AutoReadOnly` property's own name apart from an unrelated
//! local variable or parameter that happens to share it; a script that
//! doesn't parse cleanly is left unchecked rather than guessed at.

use std::collections::HashSet;

use papyrus_parser::ast::{Expr, FunctionDecl, Script, Stmt, VariableDecl};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "readonly-property-write";

/// Checks `source` for an assignment (`=`, `+=`, `-=`, ...) targeting a
/// script-level property declared `AutoReadOnly`, either by its bare name
/// or as `Self.PropertyName`. A bare name shadowed by a same-named local
/// variable or parameter in the enclosing function refers to that local/
/// parameter instead, and is never flagged. Flagged as an `[error]`, since
/// Papyrus rejects the assignment at compile time.
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

    let readonly_properties: HashSet<String> = script
        .properties
        .iter()
        .filter(|property| property.is_auto_read_only)
        .map(|property| property.name.to_ascii_lowercase())
        .collect();
    if readonly_properties.is_empty() {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        let shadowed: HashSet<String> = function
            .params
            .iter()
            .map(|param| param.name.to_ascii_lowercase())
            .chain(
                collect_var_decls(&function.body)
                    .into_iter()
                    .map(|decl| decl.name.to_ascii_lowercase()),
            )
            .collect();

        for assign in collect_assigns(&function.body) {
            let Stmt::Assign { target, line, .. } = assign else {
                continue;
            };
            let Some(target) = target_property(target) else {
                continue;
            };
            let name_lower = target.name.to_ascii_lowercase();
            if !readonly_properties.contains(&name_lower) {
                continue;
            }
            if !target.is_self_qualified && shadowed.contains(&name_lower) {
                continue;
            }

            diagnostics.push(Diagnostic {
                line: *line,
                column: 1,
                message: format!(
                    "[error] '{}' is declared AutoReadOnly and cannot be assigned a new value",
                    target.name
                ),
                rule: RULE,
            });
        }
    }
    diagnostics
}

/// A property an assignment's target expression resolves to, either by its
/// bare name or as `Self.PropertyName`.
struct PropertyTarget<'a> {
    name: &'a str,
    is_self_qualified: bool,
}

/// If `target` is a bare identifier or a `Self.PropertyName` member access,
/// returns the referenced name. Anything else (a member access on
/// something other than `Self`, an index, ...) returns `None` rather than
/// being guessed at.
fn target_property(target: &Expr) -> Option<PropertyTarget<'_>> {
    match target {
        Expr::Identifier(name) => Some(PropertyTarget {
            name,
            is_self_qualified: false,
        }),
        Expr::Member { object, property } if matches!(object.as_ref(), Expr::Self_) => {
            Some(PropertyTarget {
                name: property,
                is_self_qualified: true,
            })
        }
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
#[path = "readonly_property_write_tests.rs"]
mod tests;
