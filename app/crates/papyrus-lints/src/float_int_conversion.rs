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

use papyrus_parser::ast::{Expr, FunctionDecl, PropertyDecl, Script, Stmt, TypeName, VariableDecl};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "float-to-int";

struct IndexedFunction {
    name: String,
    params: Vec<(String, TypeName)>,
}

#[derive(Default)]
struct Collect {
    store: Store,
    env: Option<TypeEnv>,
    functions: HashMap<String, IndexedFunction>,
    return_type: Option<TypeName>,
    function_name: String,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.env = Some(TypeEnv::for_script(script));
        self.functions = index_functions(script);
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(env) = &mut self.env {
            env.enter_function(function);
        }
        self.return_type = function.return_type.clone();
        self.function_name = function.name.clone();
    }

    fn leave_function(&mut self, _function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(env) = &mut self.env {
            env.leave_function();
        }
        self.return_type = None;
        self.function_name.clear();
    }

    fn visit_property(&mut self, property: &PropertyDecl, _ctx: &mut VisitCtx<'_>) {
        let Some(value) = &property.value else {
            return;
        };
        let Some(env) = self.env.as_ref() else {
            return;
        };
        let mut diagnostics = Vec::new();
        check_declaration(
            &property.type_name,
            &property.name,
            value,
            property.line,
            env,
            &mut diagnostics,
        );
        self.store.extend(diagnostics);
    }

    fn visit_variable(&mut self, variable: &VariableDecl, _ctx: &mut VisitCtx<'_>) {
        let Some(value) = &variable.value else {
            return;
        };
        let Some(env) = self.env.as_ref() else {
            return;
        };
        let mut diagnostics = Vec::new();
        check_declaration(
            &variable.type_name,
            &variable.name,
            value,
            variable.line,
            env,
            &mut diagnostics,
        );
        self.store.extend(diagnostics);
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        let Some(env) = self.env.as_ref() else {
            return;
        };
        let mut diagnostics = Vec::new();
        match stmt {
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
                        &mut diagnostics,
                        format!(
                            "[warning] Float value assigned to Int {} without an explicit 'as Int' cast",
                            describe_target(target)
                        ),
                    );
                }
            }
            Stmt::Return {
                value: Some(value),
                line,
            } => {
                if let Some(return_type) = &self.return_type {
                    flag_narrowing(
                        return_type,
                        value,
                        env,
                        *line,
                        &mut diagnostics,
                        format!(
                            "[warning] Float value returned from Int function '{}' without an explicit 'as Int' cast",
                            self.function_name
                        ),
                    );
                }
            }
            _ => {}
        }
        self.store.extend(diagnostics);
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Expr::Call { callee, args, .. } = expr else {
            return;
        };
        let Some(env) = self.env.as_ref() else {
            return;
        };
        let mut diagnostics = Vec::new();
        check_call_args(
            callee,
            args,
            env,
            &self.functions,
            ctx.line,
            &mut diagnostics,
        );
        self.store.extend(diagnostics);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for Float values narrowed into an Int without an
/// explicit cast.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

fn index_functions(script: &Script) -> HashMap<String, IndexedFunction> {
    let mut functions = HashMap::new();
    for function in &script.functions {
        functions
            .entry(function.name.to_lowercase())
            .or_insert_with(|| index_function(function));
    }
    for state in &script.states {
        for function in &state.functions {
            functions
                .entry(function.name.to_lowercase())
                .or_insert_with(|| index_function(function));
        }
    }
    functions
}

fn index_function(function: &FunctionDecl) -> IndexedFunction {
    IndexedFunction {
        name: function.name.clone(),
        params: function
            .params
            .iter()
            .map(|param| (param.name.clone(), param.type_name.clone()))
            .collect(),
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

fn check_call_args(
    callee: &Expr,
    args: &[Expr],
    env: &TypeEnv,
    functions: &HashMap<String, IndexedFunction>,
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
        let (arg, param_name, param_type) = match arg {
            Expr::NamedArg { name, value } => {
                let Some((param_name, param_type)) = function
                    .params
                    .iter()
                    .find(|(param_name, _)| param_name.eq_ignore_ascii_case(name))
                else {
                    continue;
                };
                (value.as_ref(), param_name.as_str(), param_type)
            }
            _ => {
                let Some((param_name, param_type)) = function.params.get(index) else {
                    break;
                };
                (arg, param_name.as_str(), param_type)
            }
        };
        flag_narrowing(
            param_type,
            arg,
            env,
            line,
            diagnostics,
            format!(
                "[warning] Float value passed as Int parameter '{param_name}' of function '{}' without an explicit 'as Int' cast",
                function.name
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
