//! Flags call arguments whose inferred type doesn't match the callee's
//! declared parameter type.
//!
//! Unlike the other lints in this crate, this one works from the parsed
//! AST (via [`papyrus_parser::types`]) rather than raw tokens, since it
//! needs to know declared parameter types to say anything useful. A call
//! whose target or argument type can't be determined from the script
//! alone (an unqualified type, a member access on an unresolved object, an
//! expression this crate doesn't model) is silently skipped rather than
//! guessed at, to keep false positives rare.
//!
//! Calls to functions declared in the script being linted are always
//! checked (see [`check`]). Calls to functions declared on *other*
//! scripts (e.g. `SomeProperty.DoThing(...)`) additionally need those
//! scripts' signatures, which requires resolving script names to files —
//! something this crate deliberately has no filesystem access to do. A
//! caller that can supply such signatures (e.g. the Tauri app, backed by
//! its `FunctionTable`) can do so by implementing [`ExternalSignatures`]
//! and calling [`check_with`] instead.

use std::collections::HashMap;

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::{Diagnostic, ExternalSignatures, ParamInfo};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "argument-types";

#[allow(dead_code)] // not dispatched from collect_diagnostics yet
pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::ast()
}

/// Checks `source` for argument/parameter type mismatches on calls to
/// functions declared in the same script. Calls on other scripts' types
/// are not checked; see [`check_with`] for that.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config);
    check_with(ast, external)
}

/// Like [`check`], but also checks calls to functions resolved through
/// `external` (typically functions declared on other scripts).
pub fn check_with<E: ExternalSignatures>(
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };

    let locals = LocalFunctions::from_script(script);
    let mut env = TypeEnv::for_script(script);
    let mut diagnostics = Vec::new();

    for function in all_functions(script) {
        env.with_function_scope(function, |env| {
            for stmt in &function.body {
                walk_stmt(stmt, env, &locals, external, &mut diagnostics);
            }
        });
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

/// Parameters of the functions declared in the script being linted, keyed
/// by lowercased name. A name declared more than once (e.g. overridden in
/// a state) with a differing signature (including differing parameter
/// names, since those decide how a named argument is resolved) is stored
/// as `None`, since which declaration applies at a given call site can't
/// be determined here — such calls are then skipped rather than checked
/// against a possibly-wrong signature.
struct LocalFunctions {
    by_name: HashMap<String, Option<Vec<ParamInfo>>>,
}

impl LocalFunctions {
    fn from_script(script: &Script) -> Self {
        let mut grouped: HashMap<String, Vec<&FunctionDecl>> = HashMap::new();
        for function in all_functions(script) {
            grouped
                .entry(function.name.to_ascii_lowercase())
                .or_default()
                .push(function);
        }

        let by_name = grouped
            .into_iter()
            .map(|(name, decls)| {
                let first: Vec<ParamInfo> = decls[0]
                    .params
                    .iter()
                    .map(|p| ParamInfo {
                        name: p.name.clone(),
                        type_name: p.type_name.clone(),
                    })
                    .collect();
                let consistent = decls.iter().all(|decl| {
                    decl.params.len() == first.len()
                        && decl.params.iter().zip(&first).all(|(p, expected)| {
                            p.name.eq_ignore_ascii_case(&expected.name)
                                && p.type_name == expected.type_name
                        })
                });
                (name, consistent.then_some(first))
            })
            .collect();

        LocalFunctions { by_name }
    }

    fn lookup(&self, name: &str) -> Option<&[ParamInfo]> {
        self.by_name.get(&name.to_ascii_lowercase())?.as_deref()
    }
}

fn walk_stmt<E: ExternalSignatures>(
    stmt: &Stmt,
    env: &TypeEnv,
    locals: &LocalFunctions,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr(value, env, locals, external, diagnostics);
            }
        }
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, env, locals, external, diagnostics);
            walk_expr(value, env, locals, external, diagnostics);
        }
        Stmt::Expr { value, .. } => walk_expr(value, env, locals, external, diagnostics),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr(value, env, locals, external, diagnostics);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for IfBranch {
                condition, body, ..
            } in branches
            {
                walk_expr(condition, env, locals, external, diagnostics);
                for stmt in body {
                    walk_stmt(stmt, env, locals, external, diagnostics);
                }
            }
            for stmt in else_body {
                walk_stmt(stmt, env, locals, external, diagnostics);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, env, locals, external, diagnostics);
            for stmt in body {
                walk_stmt(stmt, env, locals, external, diagnostics);
            }
        }
    }
}

fn walk_expr<E: ExternalSignatures>(
    expr: &Expr,
    env: &TypeEnv,
    locals: &LocalFunctions,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Call {
            callee,
            args,
            line,
            col,
        } => {
            if let Some((name, params)) = resolve_signature(callee, env, locals, external) {
                check_args(
                    (*line, *col),
                    &name,
                    &params,
                    args,
                    env,
                    external,
                    diagnostics,
                );
            }
            walk_expr(callee, env, locals, external, diagnostics);
            for arg in args {
                walk_expr(arg, env, locals, external, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, env, locals, external, diagnostics);
            walk_expr(right, env, locals, external, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, env, locals, external, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, env, locals, external, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, env, locals, external, diagnostics);
            walk_expr(index, env, locals, external, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, env, locals, external, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, env, locals, external, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, env, locals, external, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Resolves `callee` to a function name and its parameters (name and
/// type), checking the script's own functions first and falling back to
/// `external` for anything that isn't a local call (or isn't declared
/// locally). Since [`ExternalSignatures`] resolves another script's
/// functions down to full parameter info now too, a named argument
/// (`func(argB = 1)`) can be matched to the parameter it fills either way
/// (see [`check_args`]).
fn resolve_signature<E: ExternalSignatures>(
    callee: &Expr,
    env: &TypeEnv,
    locals: &LocalFunctions,
    external: &mut E,
) -> Option<(String, Vec<ParamInfo>)> {
    let (object_type, function_name) = match callee {
        Expr::Identifier(name) => {
            if let Some(params) = locals.lookup(name) {
                return Some((name.clone(), params.to_vec()));
            }
            (infer_type(&Expr::Self_, env)?, name.clone())
        }
        Expr::Member { object, property } => {
            if matches!(**object, Expr::Self_) {
                if let Some(params) = locals.lookup(property) {
                    return Some((property.clone(), params.to_vec()));
                }
            }
            (infer_type(object, env)?, property.clone())
        }
        _ => return None,
    };

    if object_type.is_array {
        return None;
    }
    external
        .lookup(&object_type.name, &function_name)
        .map(|params| (function_name, params))
}

fn check_args<E: ExternalSignatures>(
    (line, col): (usize, usize),
    function_name: &str,
    params: &[ParamInfo],
    args: &[Expr],
    env: &TypeEnv,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (index, arg) in args.iter().enumerate() {
        let (param_index, arg) = match arg {
            Expr::NamedArg { name, value } => {
                let Some(param_index) = params
                    .iter()
                    .position(|p| p.name.eq_ignore_ascii_case(name))
                else {
                    continue;
                };
                (param_index, value.as_ref())
            }
            _ => (index, arg),
        };
        let Some(param_type) = params.get(param_index).map(|p| &p.type_name) else {
            break;
        };

        if matches!(arg, Expr::Literal(Literal::None)) {
            if !accepts_none(param_type) {
                diagnostics.push(mismatch(
                    line,
                    col,
                    function_name,
                    param_index,
                    param_type,
                    "None",
                ));
            }
            continue;
        }

        let Some(arg_type) = infer_type(arg, env) else {
            continue;
        };
        if !is_compatible(param_type, &arg_type, external) {
            diagnostics.push(mismatch(
                line,
                col,
                function_name,
                param_index,
                param_type,
                &format_type(&arg_type),
            ));
        }
    }
}

fn mismatch(
    line: usize,
    col: usize,
    function_name: &str,
    index: usize,
    param_type: &TypeName,
    got: &str,
) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!(
            "[error] Argument {} to '{}' expects {} but got {}",
            index + 1,
            function_name,
            format_type(param_type),
            got
        ),
        rule: RULE,
    }
}

pub(crate) fn format_type(type_name: &TypeName) -> String {
    if type_name.is_array {
        format!("{}[]", type_name.name)
    } else {
        type_name.name.clone()
    }
}

fn is_numeric(name: &str) -> bool {
    name.eq_ignore_ascii_case("int") || name.eq_ignore_ascii_case("float")
}

pub(crate) fn is_primitive(name: &str) -> bool {
    is_numeric(name) || name.eq_ignore_ascii_case("bool") || name.eq_ignore_ascii_case("string")
}

/// `None` is valid for any reference type (arrays and non-primitive
/// object types) but not for `Int`/`Float`/`Bool`/`String`.
pub(crate) fn accepts_none(param_type: &TypeName) -> bool {
    param_type.is_array || !is_primitive(&param_type.name)
}

/// Whether an argument of type `arg_type` may be passed for a parameter
/// declared as `param_type`. Exact matches (case-insensitively) are
/// always compatible; Papyrus also allows widening an `Int` argument to a
/// `Float` parameter, and passing an object whose script extends (directly
/// or transitively) the parameter's type, per `external`'s knowledge of
/// the scripts' `Extends` chains.
pub(crate) fn is_compatible<E: ExternalSignatures>(
    param_type: &TypeName,
    arg_type: &TypeName,
    external: &mut E,
) -> bool {
    if param_type.is_array != arg_type.is_array {
        return false;
    }
    if param_type.name.eq_ignore_ascii_case(&arg_type.name) {
        return true;
    }
    if !param_type.is_array
        && param_type.name.eq_ignore_ascii_case("float")
        && arg_type.name.eq_ignore_ascii_case("int")
    {
        return true;
    }
    if param_type.is_array || is_primitive(&param_type.name) || is_primitive(&arg_type.name) {
        return false;
    }
    external.is_subtype(&arg_type.name, &param_type.name)
}

#[cfg(test)]
#[path = "argument_types_tests.rs"]
mod tests;
