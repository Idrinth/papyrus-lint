//! Flags implicit Float-to-Int narrowing: a `Float` value declared,
//! assigned, returned, or passed as an argument into an `Int`-typed slot
//! without an explicit `as Int` cast.
//!
//! Unlike the other lints in this crate, this one needs to know a value's
//! inferred type, so it works on the parsed AST (see
//! `papyrus_parser::types`) rather than raw tokens. Scripts that fail to
//! parse simply aren't checked, the same way a lexer failure short-circuits
//! the token-based lints.

use std::collections::HashMap;

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "float-to-int";

/// Checks `source` for Float values narrowed into an Int without an
/// explicit cast.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::argument_types::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config, external);

    let Some(script) = ast else {
        return Vec::new();
    };

    let functions = index_functions(script);
    let mut env = TypeEnv::for_script(script);
    let mut diagnostics = Vec::new();
    check_script_declarations(script, &env, &functions, &mut diagnostics);
    check_function_bodies(script, &mut env, &functions, &mut diagnostics);
    diagnostics
}

fn index_functions(script: &Script) -> HashMap<String, &FunctionDecl> {
    let mut functions: HashMap<String, &FunctionDecl> = script
        .functions
        .iter()
        .map(|function| (function.name.to_lowercase(), function))
        .collect();
    for state in &script.states {
        for function in &state.functions {
            functions
                .entry(function.name.to_lowercase())
                .or_insert(function);
        }
    }
    functions
}

fn check_script_declarations(
    script: &Script,
    env: &TypeEnv,
    functions: &HashMap<String, &FunctionDecl>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for variable in &script.variables {
        if let Some(value) = &variable.value {
            check_declaration(
                &variable.type_name,
                &variable.name,
                value,
                variable.line,
                env,
                diagnostics,
            );
            walk_expr(value, env, functions, variable.line, diagnostics);
        }
    }
    for property in &script.properties {
        if let Some(value) = &property.value {
            check_declaration(
                &property.type_name,
                &property.name,
                value,
                property.line,
                env,
                diagnostics,
            );
            walk_expr(value, env, functions, property.line, diagnostics);
        }
    }
}

fn check_function_bodies(
    script: &Script,
    env: &mut TypeEnv,
    functions: &HashMap<String, &FunctionDecl>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for function in &script.functions {
        env.with_function_scope(function, |scoped| {
            check_body(
                &function.body,
                scoped,
                functions,
                function.return_type.as_ref(),
                &function.name,
                diagnostics,
            );
        });
    }
    for state in &script.states {
        for function in &state.functions {
            env.with_function_scope(function, |scoped| {
                check_body(
                    &function.body,
                    scoped,
                    functions,
                    function.return_type.as_ref(),
                    &function.name,
                    diagnostics,
                );
            });
        }
    }
}

fn check_body(
    body: &[Stmt],
    env: &TypeEnv,
    functions: &HashMap<String, &FunctionDecl>,
    return_type: Option<&TypeName>,
    function_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        check_stmt(
            stmt,
            env,
            functions,
            return_type,
            function_name,
            diagnostics,
        );
    }
}

fn check_stmt(
    stmt: &Stmt,
    env: &TypeEnv,
    functions: &HashMap<String, &FunctionDecl>,
    return_type: Option<&TypeName>,
    function_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                check_declaration(
                    &decl.type_name,
                    &decl.name,
                    value,
                    decl.line,
                    env,
                    diagnostics,
                );
                walk_expr(value, env, functions, decl.line, diagnostics);
            }
        }
        Stmt::Assign {
            target,
            value,
            line,
            ..
        } => {
            if let Some(target_type) = infer_type(target, env) {
                flag_narrowing(
                    &target_type,
                    value,
                    env,
                    *line,
                    diagnostics,
                    format!(
                        "[warning] Float value assigned to Int {} without an explicit 'as Int' cast",
                        describe_target(target)
                    ),
                );
            }
            walk_expr(target, env, functions, *line, diagnostics);
            walk_expr(value, env, functions, *line, diagnostics);
        }
        Stmt::Expr { value, line } => {
            walk_expr(value, env, functions, *line, diagnostics);
        }
        Stmt::Return {
            value: Some(value),
            line,
        } => {
            if let Some(return_type) = return_type {
                flag_narrowing(
                    return_type,
                    value,
                    env,
                    *line,
                    diagnostics,
                    format!(
                        "[warning] Float value returned from Int function '{function_name}' without an explicit 'as Int' cast"
                    ),
                );
            }
            walk_expr(value, env, functions, *line, diagnostics);
        }
        Stmt::Return { value: None, .. } => {}
        Stmt::If {
            branches,
            else_body,
            line,
            ..
        } => {
            for IfBranch {
                condition, body, ..
            } in branches
            {
                walk_expr(condition, env, functions, *line, diagnostics);
                check_body(
                    body,
                    env,
                    functions,
                    return_type,
                    function_name,
                    diagnostics,
                );
            }
            check_body(
                else_body,
                env,
                functions,
                return_type,
                function_name,
                diagnostics,
            );
        }
        Stmt::While {
            condition,
            body,
            line,
            ..
        } => {
            walk_expr(condition, env, functions, *line, diagnostics);
            check_body(
                body,
                env,
                functions,
                return_type,
                function_name,
                diagnostics,
            );
        }
    }
}

fn flag_narrowing(
    target_type: &TypeName,
    value: &Expr,
    env: &TypeEnv,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
    message: String,
) {
    if narrows_to_int(target_type, value, env) {
        diagnostics.push(Diagnostic {
            line,
            column: 1,
            message,
            rule: RULE,
        });
    }
}

/// Recursively walks `expr` looking for calls to functions declared in this
/// script, flagging any argument that narrows a Float into an Int
/// parameter without an explicit cast.
///
/// `line` is the enclosing statement's line, since expressions don't carry
/// their own position in this AST.
fn walk_expr(
    expr: &Expr,
    env: &TypeEnv,
    functions: &HashMap<String, &FunctionDecl>,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Expr::Call { callee, args, .. } = expr {
        check_call_args(callee, args, env, functions, line, diagnostics);
        for arg in args {
            walk_expr(arg, env, functions, line, diagnostics);
        }
        walk_expr(callee, env, functions, line, diagnostics);
        return;
    }

    match expr {
        Expr::Binary { left, right, .. } => {
            walk_expr(left, env, functions, line, diagnostics);
            walk_expr(right, env, functions, line, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, env, functions, line, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, env, functions, line, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, env, functions, line, diagnostics);
            walk_expr(index, env, functions, line, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, env, functions, line, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, env, functions, line, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, env, functions, line, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent | Expr::Call { .. } => {
        }
    }
}

fn check_call_args(
    callee: &Expr,
    args: &[Expr],
    env: &TypeEnv,
    functions: &HashMap<String, &FunctionDecl>,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let resolved_name = match callee {
        Expr::Identifier(name) => Some(name.as_str()),
        Expr::Member { object, property } if matches!(**object, Expr::Self_) => {
            Some(property.as_str())
        }
        _ => None,
    };
    let Some(name) = resolved_name else {
        return;
    };
    let Some(function) = functions.get(&name.to_lowercase()) else {
        return;
    };
    for (index, arg) in args.iter().enumerate() {
        let (arg, param) = match arg {
            Expr::NamedArg { name, value } => {
                let Some(param) = function
                    .params
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(name))
                else {
                    continue;
                };
                (value.as_ref(), param)
            }
            _ => {
                let Some(param) = function.params.get(index) else {
                    break;
                };
                (arg, param)
            }
        };
        flag_narrowing(
            &param.type_name,
            arg,
            env,
            line,
            diagnostics,
            format!(
                "[warning] Float value passed as Int parameter '{}' of function '{}' without an explicit 'as Int' cast",
                param.name, function.name
            ),
        );
    }
}

/// Flags `value` when it narrows a Float into an Int-typed declaration
/// (a variable, script-level variable, or property) named `name`.
fn check_declaration(
    type_name: &TypeName,
    name: &str,
    value: &Expr,
    line: usize,
    env: &TypeEnv,
    diagnostics: &mut Vec<Diagnostic>,
) {
    flag_narrowing(
        type_name,
        value,
        env,
        line,
        diagnostics,
        format!(
            "[warning] Float value assigned to Int variable '{name}' without an explicit 'as Int' cast"
        ),
    );
}

fn is_int(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("int")
}

fn is_float(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("float")
}

/// True when `value` is a Float being narrowed into an Int-typed `target_type`.
///
/// A value's inferred type already reflects any explicit cast it carries
/// (`someFloat as Int` infers as `Int`), so comparing the plain inferred
/// type against the target is enough to let explicit casts through.
fn narrows_to_int(target_type: &TypeName, value: &Expr, env: &TypeEnv) -> bool {
    is_int(target_type) && infer_type(value, env).is_some_and(|value_type| is_float(&value_type))
}

fn describe_target(target: &Expr) -> String {
    match target {
        Expr::Identifier(name) => format!("variable '{name}'"),
        Expr::Member { property, .. } => format!("property '{property}'"),
        Expr::Index { .. } => "array element".to_string(),
        _ => "target".to_string(),
    }
}

#[cfg(test)]
#[path = "float_int_conversion_tests.rs"]
mod tests;
