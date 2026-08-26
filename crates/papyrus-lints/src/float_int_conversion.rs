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

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Stmt, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::Diagnostic;

/// Checks `source` for Float values narrowed into an Int without an
/// explicit cast.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

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

    let mut env = TypeEnv::for_script(&script);
    let mut diagnostics = Vec::new();

    for variable in &script.variables {
        if let Some(value) = &variable.value {
            check_declaration(
                &variable.type_name,
                &variable.name,
                value,
                variable.line,
                &env,
                &mut diagnostics,
            );
            walk_expr(value, &env, &functions, variable.line, &mut diagnostics);
        }
    }
    for property in &script.properties {
        if let Some(value) = &property.value {
            check_declaration(
                &property.type_name,
                &property.name,
                value,
                property.line,
                &env,
                &mut diagnostics,
            );
            walk_expr(value, &env, &functions, property.line, &mut diagnostics);
        }
    }

    for function in &script.functions {
        env.with_function_scope(function, |scoped| {
            check_body(
                &function.body,
                scoped,
                &functions,
                function.return_type.as_ref(),
                &function.name,
                &mut diagnostics,
            );
        });
    }
    for state in &script.states {
        for function in &state.functions {
            env.with_function_scope(function, |scoped| {
                check_body(
                    &function.body,
                    scoped,
                    &functions,
                    function.return_type.as_ref(),
                    &function.name,
                    &mut diagnostics,
                );
            });
        }
    }

    diagnostics
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
                    if narrows_to_int(&target_type, value, env) {
                        diagnostics.push(Diagnostic {
                            line: *line,
                            column: 1,
                            message: format!(
                                "Float value assigned to Int {} without an explicit 'as Int' cast",
                                describe_target(target)
                            ),
                        });
                    }
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
                    if narrows_to_int(return_type, value, env) {
                        diagnostics.push(Diagnostic {
                            line: *line,
                            column: 1,
                            message: format!(
                                "Float value returned from Int function '{function_name}' without an explicit 'as Int' cast"
                            ),
                        });
                    }
                }
                walk_expr(value, env, functions, *line, diagnostics);
            }
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                line,
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
        let resolved_name = match &**callee {
            Expr::Identifier(name) => Some(name.as_str()),
            Expr::Member { object, property } if matches!(**object, Expr::Self_) => {
                Some(property.as_str())
            }
            _ => None,
        };
        if let Some(name) = resolved_name {
            if let Some(function) = functions.get(&name.to_lowercase()) {
                for (arg, param) in args.iter().zip(&function.params) {
                    if narrows_to_int(&param.type_name, arg, env) {
                        diagnostics.push(Diagnostic {
                            line,
                            column: 1,
                            message: format!(
                                "Float value passed as Int parameter '{}' of function '{}' without an explicit 'as Int' cast",
                                param.name, function.name
                            ),
                        });
                    }
                }
            }
        }
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
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent | Expr::Call { .. } => {
        }
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
    if narrows_to_int(type_name, value, env) {
        diagnostics.push(Diagnostic {
            line,
            column: 1,
            message: format!(
                "Float value assigned to Int variable '{name}' without an explicit 'as Int' cast"
            ),
        });
    }
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
mod tests {
    use super::*;

    #[test]
    fn flags_float_literal_in_int_declaration() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Int x = 1.5\nEndFunction\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.contains("'x'"));
    }

    #[test]
    fn flags_float_variable_assigned_to_int_variable() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = 1.5\n    Int x = 0\n    x = f\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
        assert!(diagnostics[0].message.contains("variable 'x'"));
    }

    #[test]
    fn flags_compound_assignment_that_narrows() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = 0.5\n    Int x = 1\n    x += f\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn does_not_flag_explicit_cast_to_int() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = 1.5\n    Int x = f as Int\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_int_to_int_or_float_to_float() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 1\n    Int b = a\n    Float c = 1.5\n    Float d = c\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_int_widening_to_float() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 1\n    Float f = a\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_float_returned_from_int_function() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test()\n    Float f = 1.5\n    Return f\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Test"));
    }

    #[test]
    fn does_not_flag_explicit_cast_in_return() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test()\n    Float f = 1.5\n    Return f as Int\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_float_argument_passed_to_int_parameter_of_local_function() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Int amount)\nEndFunction\n\nFunction Test()\n    Float f = 1.5\n    Add(f)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'amount'"));
        assert!(diagnostics[0].message.contains("'Add'"));
    }

    #[test]
    fn does_not_flag_float_argument_when_param_is_float() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Float amount)\nEndFunction\n\nFunction Test()\n    Float f = 1.5\n    Add(f)\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_narrowing_nested_inside_a_call_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Add(Int amount)\n    Return 0\nEndFunction\n\nFunction Outer(Float amount)\nEndFunction\n\nFunction Test()\n    Float f = 1.5\n    Outer(Add(f))\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'Add'"));
    }

    #[test]
    fn flags_float_literal_in_script_level_variable_and_property() {
        let diagnostics =
            check("ScriptName Example\n\nInt _count = 1.5\nInt Property Total = 2.5 Auto\n");
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|d| d.message.contains("'_count'")));
        assert!(diagnostics.iter().any(|d| d.message.contains("'Total'")));
    }

    #[test]
    fn flags_float_argument_passed_via_self_qualified_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Int amount)\nEndFunction\n\nFunction Test()\n    Float f = 1.5\n    self.Add(f)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'amount'"));
    }

    #[test]
    fn flags_float_argument_passed_to_state_function() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = 1.5\n    Add(f)\nEndFunction\n\nState Active\n    Function Add(Int amount)\n    EndFunction\nEndState\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'amount'"));
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }
}
