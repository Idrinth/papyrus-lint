//! Flags implicit comparisons between different numeric types: an `Int`
//! value compared against a `Float` value (`==`, `!=`, `<`, `<=`, `>`, `>=`)
//! without an explicit cast making the comparison exact.
//!
//! Papyrus implicitly widens the `Int` side to `Float` for such a
//! comparison, which can produce surprising results (particularly for
//! `==`/`!=`) once floating-point precision is involved.
//!
//! Like the other type-aware lints in this crate, this one works on the
//! parsed AST (see `papyrus_parser::types`) rather than raw tokens. Scripts
//! that fail to parse simply aren't checked.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Script, Stmt, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::Diagnostic;

/// Checks `source` for `Int`/`Float` comparisons that aren't already made
/// exact by an explicit cast.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut env = TypeEnv::for_script(&script);
    let mut diagnostics = Vec::new();

    for function in all_functions(&script) {
        env.with_function_scope(function, |scoped| {
            check_body(&function.body, scoped, &mut diagnostics);
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

fn check_body(body: &[Stmt], env: &TypeEnv, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    walk_expr(value, env, decl.line, diagnostics);
                }
            }
            Stmt::Assign {
                target,
                value,
                line,
                ..
            } => {
                walk_expr(target, env, *line, diagnostics);
                walk_expr(value, env, *line, diagnostics);
            }
            Stmt::Expr { value, line } => walk_expr(value, env, *line, diagnostics),
            Stmt::Return {
                value: Some(value),
                line,
            } => {
                walk_expr(value, env, *line, diagnostics);
            }
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
                    walk_expr(condition, env, *line, diagnostics);
                    check_body(body, env, diagnostics);
                }
                check_body(else_body, env, diagnostics);
            }
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                walk_expr(condition, env, *line, diagnostics);
                check_body(body, env, diagnostics);
            }
        }
    }
}

/// Recursively walks `expr` looking for `Int`/`Float` comparisons.
///
/// `line` is the enclosing statement's line, since expressions don't carry
/// their own position in this AST.
fn walk_expr(expr: &Expr, env: &TypeEnv, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    if let Expr::Binary { left, op, right } = expr {
        if is_comparison(*op) {
            check_comparison(left, right, env, line, diagnostics);
        }
        walk_expr(left, env, line, diagnostics);
        walk_expr(right, env, line, diagnostics);
        return;
    }

    match expr {
        Expr::Unary { operand, .. } => walk_expr(operand, env, line, diagnostics),
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, env, line, diagnostics);
            for arg in args {
                walk_expr(arg, env, line, diagnostics);
            }
        }
        Expr::Member { object, .. } => walk_expr(object, env, line, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, env, line, diagnostics);
            walk_expr(index, env, line, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, env, line, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, env, line, diagnostics),
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::Binary { .. } => {}
    }
}

fn is_comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Gt
            | BinaryOp::Lt
            | BinaryOp::GtEq
            | BinaryOp::LtEq
    )
}

fn is_int(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("int")
}

fn is_float(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("float")
}

/// Flags `left op right` when one side is `Int` and the other `Float`.
///
/// A side's inferred type already reflects any explicit cast it carries
/// (`someFloat as Int` infers as `Int`), so comparing the plain inferred
/// types is enough to let explicit casts through.
fn check_comparison(
    left: &Expr,
    right: &Expr,
    env: &TypeEnv,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(left_ty) = infer_type(left, env) else {
        return;
    };
    let Some(right_ty) = infer_type(right, env) else {
        return;
    };

    let mismatched =
        (is_int(&left_ty) && is_float(&right_ty)) || (is_float(&left_ty) && is_int(&right_ty));
    if mismatched {
        diagnostics.push(Diagnostic {
            line,
            column: 1,
            message: "Comparison between Int and Float without an explicit cast; \
                      floating-point precision may make this inexact"
                .to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_int_variable_compared_to_float_literal() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1.0\n    EndIf\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.contains("Int and Float"));
    }

    #[test]
    fn flags_float_variable_compared_to_int_variable_with_every_comparison_operator() {
        let diagnostics = check(
            r#"
ScriptName Example

Function Test(Int a, Float f)
    If a == f
    EndIf
    If a != f
    EndIf
    If a < f
    EndIf
    If a <= f
    EndIf
    If a > f
    EndIf
    If a >= f
    EndIf
EndFunction
"#,
        );
        assert_eq!(diagnostics.len(), 6);
    }

    #[test]
    fn flags_mismatched_comparison_outside_a_condition() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Float f)\n    Bool result = a == f\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn does_not_flag_int_to_int_or_float_to_float_comparisons() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Int b, Float c, Float d)\n    If a == b\n    EndIf\n    If c == d\n    EndIf\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_comparison_with_an_explicit_cast() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Float f)\n    If a == f as Int\n    EndIf\n    If (a as Float) == f\n    EndIf\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_comparisons_whose_type_cannot_be_resolved_locally() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a)\n    If a == GetValue()\n    EndIf\n    If a == Self.SomeProperty\n    EndIf\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_nested_and_state_function_bodies() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Int a, Float f)\n        If a > 0\n            If a == f\n            EndIf\n        EndIf\n    EndFunction\nEndState\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }
}
