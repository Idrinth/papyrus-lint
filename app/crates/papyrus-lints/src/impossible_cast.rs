//! Flags an explicit `as` cast that can never succeed: the value's known
//! type and the cast's target type are *proven* unrelated — neither
//! extends the other, directly or transitively — so the cast always
//! evaluates to `None` at runtime no matter what the value actually holds
//! (e.g. `Armor a` then `Weapon b = a as Weapon`, since `Armor` and
//! `Weapon` are unrelated siblings both directly extending `Form`).
//!
//! Like [`crate::useless_downcast`], this works from the parsed AST (via
//! [`papyrus_parser::types`]) to know a cast's value's declared type, and
//! only checks a cast whose value's type can be determined locally
//! (locals, parameters, properties, `Self`/`Parent`, literals, and other
//! resolvable expressions) — a member access or function call result is
//! left unflagged rather than guessed at. Primitive types (`Int`, `Float`,
//! `Bool`, `String`) are never flagged, since Papyrus's conversions between
//! those (and between a primitive and an object type) are a different
//! concern entirely from object-type subtyping.
//!
//! Papyrus scripts have single inheritance, so two types are unrelated
//! exactly when neither's `Extends` chain reaches the other — but a
//! negative [`ExternalSignatures::is_subtype`] result alone doesn't prove
//! that: it's also what an *unresolvable* chain (an unknown type this
//! crate simply has no data for) returns, and flagging on that would be
//! guessing. [`ExternalSignatures::ancestry_fully_known`] is what
//! distinguishes the two: a cast is only ever flagged once both the
//! value's and the target's `Extends` chains are confirmed to resolve all
//! the way to a definite root (a script with no `Extends` at all, or a
//! native engine type from `rules/native-types.yaml` with no further
//! parent) without ever reaching each other.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::argument_types::{is_primitive, ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "impossible-cast";

/// Checks `source` for an `as` cast proven impossible using only
/// same-script information. Since that alone can never confirm a type's
/// full ancestry resolves to a definite root (see the module docs), this
/// never actually flags anything on its own — see [`check_with`].
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but resolves both the value's and the target's full
/// `Extends` ancestry through `external`, the same way
/// [`crate::useless_downcast::check_with`] resolves ancestor-type casts.
pub fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut env = TypeEnv::for_script(&script);
    let mut diagnostics = Vec::new();

    for function in all_functions(&script) {
        env.with_function_scope(function, |env| {
            for stmt in &function.body {
                walk_stmt(stmt, env, external, &mut diagnostics);
            }
        });
    }

    diagnostics
}

fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

fn walk_stmt<E: ExternalSignatures>(
    stmt: &Stmt,
    env: &TypeEnv,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr(value, env, external, decl.line, diagnostics);
            }
        }
        Stmt::Assign {
            target,
            value,
            line,
            ..
        } => {
            walk_expr(target, env, external, *line, diagnostics);
            walk_expr(value, env, external, *line, diagnostics);
        }
        Stmt::Expr { value, line } => walk_expr(value, env, external, *line, diagnostics),
        Stmt::Return {
            value: Some(value),
            line,
        } => walk_expr(value, env, external, *line, diagnostics),
        Stmt::Return { value: None, .. } => {}
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for IfBranch {
                condition,
                body,
                line,
                ..
            } in branches
            {
                walk_expr(condition, env, external, *line, diagnostics);
                for stmt in body {
                    walk_stmt(stmt, env, external, diagnostics);
                }
            }
            for stmt in else_body {
                walk_stmt(stmt, env, external, diagnostics);
            }
        }
        Stmt::While {
            condition,
            body,
            line,
            ..
        } => {
            walk_expr(condition, env, external, *line, diagnostics);
            for stmt in body {
                walk_stmt(stmt, env, external, diagnostics);
            }
        }
    }
}

fn walk_expr<E: ExternalSignatures>(
    expr: &Expr,
    env: &TypeEnv,
    external: &mut E,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Cast { value, type_name } => {
            walk_expr(value, env, external, line, diagnostics);
            if let Some(value_type) = infer_type(value, env) {
                if !value_type.is_array && impossible(&value_type.name, type_name, external) {
                    diagnostics.push(Diagnostic {
                        line,
                        column: 1,
                        message: format!(
                            "[warning] cast to '{type_name}' can never succeed: '{}' and '{type_name}' are unrelated types, so this always evaluates to None",
                            value_type.name
                        ),
                        rule: RULE,
                    });
                }
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, env, external, line, diagnostics);
            walk_expr(right, env, external, line, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, env, external, line, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, env, external, line, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, env, external, line, diagnostics);
            walk_expr(index, env, external, line, diagnostics);
        }
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, env, external, line, diagnostics);
            for arg in args {
                walk_expr(arg, env, external, line, diagnostics);
            }
        }
        Expr::NewArray { size, .. } => walk_expr(size, env, external, line, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, env, external, line, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Whether a cast from `value_type_name` to `target_type_name` is proven
/// impossible: neither extends the other (an exact match is handled by
/// [`ExternalSignatures::is_subtype`] returning `true` for equal names),
/// neither is a primitive type, and both types' full `Extends` ancestry is
/// confirmed to resolve to a definite root per `external`, per the module
/// docs.
fn impossible<E: ExternalSignatures>(
    value_type_name: &str,
    target_type_name: &str,
    external: &mut E,
) -> bool {
    if is_primitive(value_type_name) || is_primitive(target_type_name) {
        return false;
    }
    if external.is_subtype(value_type_name, target_type_name)
        || external.is_subtype(target_type_name, value_type_name)
    {
        return false;
    }
    external.ancestry_fully_known(value_type_name)
        && external.ancestry_fully_known(target_type_name)
}

#[cfg(test)]
#[path = "impossible_cast_tests.rs"]
mod tests;
