//! Flags local variables that are declared but never read back: either
//! never referenced again at all ("unused"), or only ever assigned a new
//! value without that value ever being read ("write-only").
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to tell a variable's declaration and write sites apart from an actual
//! read of it; a script that doesn't parse cleanly is left unchecked
//! rather than guessed at. Papyrus has no block scoping — a local
//! declared inside an `If`/`While` body stays valid for the rest of its
//! function — so a variable's declaration and every reference to it are
//! matched across its whole enclosing function, by name,
//! case-insensitively (as Papyrus identifiers are). Function parameters
//! and script properties aren't locals and are never flagged here.

use std::collections::HashMap;

use papyrus_parser::ast::{AssignOp, Expr, FunctionDecl, Script, Stmt, VariableDecl};

use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-local-variable";

#[allow(dead_code)] // not dispatched from collect_diagnostics yet
pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::ast()
}

/// Checks `source` for local variable declarations whose value is never
/// read anywhere in their enclosing function. Flagged as a `[warning]`.
///
/// A declaration inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers, is
/// never flagged: it's CreationKit-generated boilerplate the user can't
/// edit or remove, so whether it's read from their own code isn't
/// something they can act on.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (tokens, config, external);

    let Some(script) = ast else {
        return Vec::new();
    };
    let protected = fragment_code::protected_lines(source);

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        check_function(function, &protected, &mut diagnostics);
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

#[derive(Debug, Default, Clone, Copy)]
struct Usage {
    read: bool,
    written_after_declaration: bool,
}

fn check_function(function: &FunctionDecl, protected: &[bool], diagnostics: &mut Vec<Diagnostic>) {
    let decls = collect_var_decls(&function.body);
    if decls.is_empty() {
        return;
    }

    let mut usage: HashMap<String, Usage> = HashMap::new();
    for decl in &decls {
        usage.entry(decl.name.to_lowercase()).or_default();
    }
    walk_body(&function.body, &mut usage);

    for decl in decls {
        if protected.get(decl.line).copied().unwrap_or(false) {
            continue;
        }

        let info = usage
            .get(&decl.name.to_lowercase())
            .copied()
            .unwrap_or_default();
        if info.read {
            continue;
        }

        let message = if info.written_after_declaration {
            format!(
                "[warning] Local variable '{}' is assigned a value but never used",
                decl.name
            )
        } else {
            format!(
                "[warning] Local variable '{}' is declared but never used",
                decl.name
            )
        };

        diagnostics.push(Diagnostic {
            line: decl.line,
            column: 1,
            message,
            rule: RULE,
        });
    }
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

fn walk_body(body: &[Stmt], usage: &mut HashMap<String, Usage>) {
    for stmt in body {
        walk_stmt(stmt, usage);
    }
}

fn walk_stmt(stmt: &Stmt, usage: &mut HashMap<String, Usage>) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr_as_read(value, usage);
            }
        }
        Stmt::Assign {
            target, op, value, ..
        } => {
            walk_expr_as_read(value, usage);
            match (target, op) {
                // A plain `x = ...` overwrites x without reading its
                // previous value, so it's a write rather than a use.
                (Expr::Identifier(name), AssignOp::Assign) => {
                    if let Some(entry) = usage.get_mut(&name.to_lowercase()) {
                        entry.written_after_declaration = true;
                    }
                }
                // A compound assignment (`x += 1`, ...) reads the current
                // value of x before writing the new one.
                (Expr::Identifier(name), _) => {
                    if let Some(entry) = usage.get_mut(&name.to_lowercase()) {
                        entry.read = true;
                        entry.written_after_declaration = true;
                    }
                }
                // A member/index assignment target (`foo.Bar = 1`,
                // `arr[0] = 1`) reads whatever local it's built from to
                // resolve the member/element being assigned into.
                _ => walk_expr_as_read(target, usage),
            }
        }
        Stmt::Expr { value, .. } => walk_expr_as_read(value, usage),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr_as_read(value, usage);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for branch in branches {
                walk_expr_as_read(&branch.condition, usage);
                walk_body(&branch.body, usage);
            }
            walk_body(else_body, usage);
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr_as_read(condition, usage);
            walk_body(body, usage);
        }
    }
}

fn walk_expr_as_read(expr: &Expr, usage: &mut HashMap<String, Usage>) {
    match expr {
        Expr::Identifier(name) => {
            if let Some(entry) = usage.get_mut(&name.to_lowercase()) {
                entry.read = true;
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr_as_read(left, usage);
            walk_expr_as_read(right, usage);
        }
        Expr::Unary { operand, .. } => walk_expr_as_read(operand, usage),
        Expr::Call { callee, args, .. } => {
            walk_expr_as_read(callee, usage);
            for arg in args {
                walk_expr_as_read(arg, usage);
            }
        }
        Expr::Member { object, .. } => walk_expr_as_read(object, usage),
        Expr::Index { object, index } => {
            walk_expr_as_read(object, usage);
            walk_expr_as_read(index, usage);
        }
        Expr::Cast { value, .. } => walk_expr_as_read(value, usage),
        Expr::NewArray { size, .. } => walk_expr_as_read(size, usage),
        Expr::NamedArg { value, .. } => walk_expr_as_read(value, usage),
        Expr::Literal(_) | Expr::Self_ | Expr::Parent => {}
    }
}

#[cfg(test)]
#[path = "unused_local_variable_tests.rs"]
mod tests;
