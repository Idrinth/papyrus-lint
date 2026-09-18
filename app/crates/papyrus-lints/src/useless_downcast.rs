//! Flags an explicit `as` cast that can't narrow anything: either the
//! target type is exactly the value's already-known type, or the value's
//! known type already extends the target (directly or transitively), so
//! Papyrus would already accept the value there without the cast at all
//! (e.g. `Actor dude` then `Foo(dude as ObjectReference)`, since `Actor`
//! already extends `ObjectReference`). Such a cast never changes what the
//! expression evaluates to and only obscures the value's real type.
//!
//! Like [`crate::argument_types`], this works from the parsed AST (via
//! [`papyrus_parser::types`]) to know a cast's value's declared type, and
//! only checks a cast whose value's type can be determined locally
//! (locals, parameters, properties, `Self`/`Parent`, literals, and other
//! resolvable expressions) — a member access or function call result is
//! left unflagged rather than guessed at. Primitive types (`Int`, `Float`,
//! `Bool`, `String`) are only flagged for an exact-type cast, never for a
//! narrower relationship, since Papyrus has no subtyping between them and
//! `is_subtype` never claims one; this also keeps a meaningful conversion
//! like an explicit `Int`-to-`Float` widening cast unflagged.
//!
//! Determining that a cast target is an *ancestor* of the value's type
//! (rather than an exact match) needs to resolve the value's script's
//! `Extends` chain, which may reach into other scripts; see [`check_with`]
//! and [`crate::argument_types::ExternalSignatures::is_subtype`].

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::argument_types::{is_primitive, ExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "useless-downcast";

/// Checks `source` for a redundant `as` cast, only recognizing an
/// exact-type match (see the module docs for why a same-script check alone
/// can't recognize an ancestor-type cast as redundant too).
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::argument_types::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config);
    check_with(ast, external)
}

/// Like [`check`], but also resolves a cast target that's an ancestor
/// (rather than an exact match) of the value's known type through
/// `external`, the same way [`crate::argument_types::check_with`] resolves
/// argument subtyping.
pub fn check_with<E: ExternalSignatures>(
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };

    let mut env = TypeEnv::for_script(script);
    let mut diagnostics = Vec::new();

    for function in all_functions(script) {
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
                if !value_type.is_array {
                    if let Some(reason) = useless_reason(&value_type.name, type_name, external) {
                        diagnostics.push(Diagnostic {
                            line,
                            column: 1,
                            message: format!("[info] cast to '{type_name}' is redundant; {reason}"),
                            rule: RULE,
                        });
                    }
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

/// If a cast from `value_type_name` to `target_type_name` can't narrow
/// anything, returns a human-readable reason why; otherwise `None`. An
/// exact match (case-insensitive) is always useless. A cast between two
/// object types is also useless when `value_type_name` already extends
/// `target_type_name` (directly or transitively), per `external`; neither
/// side may be a primitive type, since Papyrus's only conversion between
/// those (`Int` to `Float`) is a meaningful, non-identity change of
/// representation, not a no-op.
fn useless_reason<E: ExternalSignatures>(
    value_type_name: &str,
    target_type_name: &str,
    external: &mut E,
) -> Option<String> {
    if value_type_name.eq_ignore_ascii_case(target_type_name) {
        return Some(format!("the value is already of type '{value_type_name}'"));
    }
    if is_primitive(value_type_name) || is_primitive(target_type_name) {
        return None;
    }
    if external.is_subtype(value_type_name, target_type_name) {
        return Some(format!(
            "'{value_type_name}' already extends '{target_type_name}'"
        ));
    }
    None
}

#[cfg(test)]
#[path = "useless_downcast_tests.rs"]
mod tests;
