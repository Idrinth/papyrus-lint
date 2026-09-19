//! Flags an `Int / Int` division whose result is then widened into a
//! `Float`-typed declaration, assignment, return, or argument.
//!
//! Papyrus evaluates `/` between two `Int`s as integer division *before*
//! any widening happens, so `Float x = 1 / 2` yields `0.0` rather than
//! `0.5` — the truncation already happened by the time the `Int` result
//! widens into the `Float` slot. Writing either operand as a `Float`
//! (`1.0 / 2`) avoids it. When both operands are compile-time-constant
//! integer literals *and* the division happens to divide evenly (e.g.
//! `72 / 8`), no truncation actually occurs, so that case is left
//! unflagged rather than reported as a false positive.
//!
//! Like [`crate::float_int_conversion`], this needs a value's inferred
//! type, so it works on the parsed AST (see `papyrus_parser::types`)
//! rather than raw tokens. Scripts that fail to parse simply aren't
//! checked, the same way a lexer failure short-circuits the token-based
//! lints.

use std::collections::HashMap;

use papyrus_parser::ast::{
    BinaryOp, Expr, FunctionDecl, Literal, PropertyDecl, Script, Stmt, TypeName, UnaryOp,
    VariableDecl,
};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "int-division-to-float";

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
                    if is_float(&target_type) {
                        flag_int_divisions(
                            value,
                            env,
                            *line,
                            &mut diagnostics,
                            format!("assigned to Float {}", describe_target(target)),
                        );
                    }
                }
            }
            Stmt::Return {
                value: Some(value),
                line,
            } => {
                if let Some(return_type) = &self.return_type {
                    if is_float(return_type) {
                        flag_int_divisions(
                            value,
                            env,
                            *line,
                            &mut diagnostics,
                            format!("returned from Float function '{}'", self.function_name),
                        );
                    }
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

const MESSAGE: &str = "Int/Int division truncates its result before it widens into a Float; \
                        write one operand as a Float (e.g. 1.0 / x) to keep the fractional result";

/// Checks `source` for an `Int / Int` division whose result is widened
/// into a Float without either operand already being a Float.
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

fn flag_int_divisions(
    value: &Expr,
    env: &TypeEnv,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
    context: String,
) {
    for _ in 0..count_int_divisions(value, env) {
        diagnostics.push(Diagnostic {
            line,
            column: 1,
            message: format!("[warning] {MESSAGE} ({context})"),
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
        if is_float(param_type) {
            flag_int_divisions(
                arg,
                env,
                line,
                diagnostics,
                format!(
                    "passed as Float parameter '{param_name}' of function '{}'",
                    function.name
                ),
            );
        }
    }
}

/// Flags `value` when it widens an Int/Int division into a Float-typed
/// declaration (a variable, script-level variable, or property) named
/// `name`.
fn check_declaration(
    type_name: &TypeName,
    name: &str,
    value: &Expr,
    line: usize,
    env: &TypeEnv,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !is_float(type_name) {
        return;
    }
    flag_int_divisions(
        value,
        env,
        line,
        diagnostics,
        format!("assigned to Float variable '{name}'"),
    );
}

fn is_int(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("int")
}

fn is_float(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("float")
}

fn is_int_expr(expr: &Expr, env: &TypeEnv) -> bool {
    infer_type(expr, env).is_some_and(|value_type| is_int(&value_type))
}

/// Counts every `Int / Int` division reachable from `expr` through
/// arithmetic operators, unary negation, casts, and named arguments —
/// the constructs that keep `expr` part of the same arithmetic result —
/// without crossing into a nested call's own arguments, an index/member
/// access, or a `new` array, which each establish their own independent
/// type context unrelated to whatever `expr` as a whole widens into.
fn count_int_divisions(expr: &Expr, env: &TypeEnv) -> usize {
    let mut count = 0;
    collect_int_divisions(expr, env, &mut count);
    count
}

fn collect_int_divisions(expr: &Expr, env: &TypeEnv, count: &mut usize) {
    if let Expr::Binary {
        left,
        op: BinaryOp::Div,
        right,
    } = expr
    {
        if is_int_expr(left, env) && is_int_expr(right, env) && !divides_evenly(left, right) {
            *count += 1;
        }
        collect_int_divisions(left, env, count);
        collect_int_divisions(right, env, count);
        return;
    }

    match expr {
        Expr::Binary { left, right, .. } => {
            collect_int_divisions(left, env, count);
            collect_int_divisions(right, env, count);
        }
        Expr::Unary { operand, .. } => collect_int_divisions(operand, env, count),
        Expr::Cast { value, .. } => collect_int_divisions(value, env, count),
        Expr::NamedArg { value, .. } => collect_int_divisions(value, env, count),
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::Call { .. }
        | Expr::Member { .. }
        | Expr::Index { .. }
        | Expr::NewArray { .. } => {}
    }
}

/// True when `left / right` is a division between two compile-time-constant
/// integer literals that happens to divide evenly, so widening its result
/// into a Float loses nothing — e.g. `72 / 8` (which is `9`, not truncated
/// from something like `9.14...`). Mirrors the conservative folding in
/// `division_by_zero`: only literals, negation, and `+`/`-`/`*` of
/// already-folded operands are folded, never another division/modulo, and
/// anything that depends on an identifier, a call, `Self`/`Parent`, a
/// member/index access, a cast, or a `new` array is left unresolved (and so
/// still flagged, since we can't tell whether it divides evenly).
fn divides_evenly(left: &Expr, right: &Expr) -> bool {
    let Some(a) = fold_int_literal(left) else {
        return false;
    };
    let Some(b) = fold_int_literal(right) else {
        return false;
    };
    b != 0 && a % b == 0
}

fn fold_int_literal(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Literal(Literal::Int { value, .. }) => Some(*value),
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => fold_int_literal(operand)?.checked_neg(),
        Expr::Binary {
            left,
            op: op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul),
            right,
        } => {
            let a = fold_int_literal(left)?;
            let b = fold_int_literal(right)?;
            match op {
                BinaryOp::Add => a.checked_add(b),
                BinaryOp::Sub => a.checked_sub(b),
                BinaryOp::Mul => a.checked_mul(b),
                _ => unreachable!(),
            }
        }
        _ => None,
    }
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
#[path = "int_division_to_float_tests.rs"]
mod tests;
