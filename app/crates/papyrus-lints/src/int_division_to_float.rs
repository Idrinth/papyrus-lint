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
    BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Stmt, TypeName, UnaryOp,
};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "int-division-to-float";

const MESSAGE: &str = "Int/Int division truncates its result before it widens into a Float; \
                        write one operand as a Float (e.g. 1.0 / x) to keep the fractional result";

/// Checks `source` for an `Int / Int` division whose result is widened
/// into a Float without either operand already being a Float.
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
                    if is_float(&target_type) {
                        for _ in 0..count_int_divisions(value, env) {
                            diagnostics.push(Diagnostic {
                                line: *line,
                                column: 1,
                                message: format!(
                                    "[warning] {MESSAGE} (assigned to Float {})",
                                    describe_target(target)
                                ),
                                rule: RULE,
                            });
                        }
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
                    if is_float(return_type) {
                        for _ in 0..count_int_divisions(value, env) {
                            diagnostics.push(Diagnostic {
                                line: *line,
                                column: 1,
                                message: format!(
                                    "[warning] {MESSAGE} (returned from Float function '{function_name}')"
                                ),
                                rule: RULE,
                            });
                        }
                    }
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
}

/// Recursively walks `expr` looking for calls to functions declared in this
/// script, flagging any argument that widens an Int/Int division into a
/// Float parameter.
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
                    if is_float(&param.type_name) {
                        for _ in 0..count_int_divisions(arg, env) {
                            diagnostics.push(Diagnostic {
                                line,
                                column: 1,
                                message: format!(
                                    "[warning] {MESSAGE} (passed as Float parameter '{}' of function '{}')",
                                    param.name, function.name
                                ),
                                rule: RULE,
                            });
                        }
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
        Expr::NamedArg { value, .. } => walk_expr(value, env, functions, line, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent | Expr::Call { .. } => {
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
    for _ in 0..count_int_divisions(value, env) {
        diagnostics.push(Diagnostic {
            line,
            column: 1,
            message: format!("[warning] {MESSAGE} (assigned to Float variable '{name}')"),
            rule: RULE,
        });
    }
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
mod tests {
    use super::*;

    #[test]
    fn flags_int_division_in_float_declaration() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Float f = 1 / 2\nEndFunction\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.contains("'f'"));
    }

    #[test]
    fn does_not_flag_constant_int_division_that_divides_evenly() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Float a = 72 / 8\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_negative_constant_int_division_that_divides_evenly() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Float a = -72 / 8\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn still_flags_constant_int_division_that_truncates() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Float a = 72 / 7\nEndFunction\n");
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn still_flags_int_division_when_an_operand_is_not_a_constant() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int x)\n    Float a = 72 / x\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_division_with_a_float_operand() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Float f = 1.0 / 2\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_when_one_operand_is_explicitly_cast_to_float() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = (1 as Float) / 2\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn still_flags_when_the_whole_division_is_cast_to_float() {
        // Casting the *result* of an Int/Int division doesn't undo the
        // truncation that already happened; only casting an operand
        // beforehand does.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = (1 / 2) as Float\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_plain_int_widening_with_no_division() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 5\n    Float f = a\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_int_division_assigned_to_an_int_target() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Int i = 1 / 2\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_modulo_between_two_ints() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Float f = 5 % 2\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_int_division_assigned_to_a_float_variable() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = 0.0\n    f = 1 / 2\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert!(diagnostics[0].message.contains("variable 'f'"));
    }

    #[test]
    fn flags_compound_assignment_that_widens() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = 0.0\n    f += 1 / 2\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn flags_int_division_returned_from_float_function() {
        let diagnostics =
            check("ScriptName Example\n\nFloat Function Test()\n    Return 1 / 2\nEndFunction\n");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'Test'"));
    }

    #[test]
    fn does_not_flag_return_of_a_float_operand_division() {
        let diagnostics =
            check("ScriptName Example\n\nFloat Function Test()\n    Return 1.0 / 2\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_int_division_passed_to_float_parameter_of_local_function() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Float amount)\nEndFunction\n\nFunction Test()\n    Add(1 / 2)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'amount'"));
        assert!(diagnostics[0].message.contains("'Add'"));
    }

    #[test]
    fn flags_int_division_passed_via_named_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Float first, Float amount)\nEndFunction\n\nFunction Test()\n    Add(1.0, amount = 1 / 2)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'amount'"));
    }

    #[test]
    fn does_not_flag_int_division_when_param_is_int() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Int amount)\nEndFunction\n\nFunction Test()\n    Add(1 / 2)\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_int_division_passed_via_self_qualified_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Add(Float amount)\nEndFunction\n\nFunction Test()\n    self.Add(1 / 2)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'amount'"));
    }

    #[test]
    fn flags_int_division_passed_to_state_function() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Add(1 / 2)\nEndFunction\n\nState Active\n    Function Add(Float amount)\n    EndFunction\nEndState\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'amount'"));
    }

    #[test]
    fn flags_each_int_division_in_one_expression_separately() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f = 1 / 2 + 3 / 4\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|d| d.line == 4));
    }

    #[test]
    fn flags_int_division_in_script_level_variable_and_property() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat _ratio = 1 / 2\nFloat Property Total = 3 / 4 Auto\n",
        );
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|d| d.message.contains("'_ratio'")));
        assert!(diagnostics.iter().any(|d| d.message.contains("'Total'")));
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }
}
