//! Flags `If`/`ElseIf`/`While` conditions that are compile-time constants
//! (e.g. `If true`, `If 1 == 2`), so they always take (or always skip) the
//! branch/loop they guard regardless of runtime state.
//!
//! Like the other AST-based lints in this crate, this one only looks at
//! expressions built entirely from literals (optionally combined with
//! arithmetic, comparison, logical, and unary operators); an expression
//! that references an identifier, a call, `Self`/`Parent`, a member/index
//! access, a cast, or a `new` array is left unflagged rather than guessed
//! at, since its value can't be known without running the script.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, UnaryOp};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "static-condition";

/// Checks every `If`/`ElseIf`/`While` condition in `source` and flags the
/// ones that evaluate to a constant `true` or `false` regardless of
/// runtime state.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        check_body(&function.body, &mut diagnostics);
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

fn check_body(body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch {
                    condition,
                    body,
                    line,
                    col,
                } in branches
                {
                    check_condition(condition, *line, *col, diagnostics);
                    check_body(body, diagnostics);
                }
                check_body(else_body, diagnostics);
            }
            Stmt::While {
                condition,
                body,
                line,
                col,
            } => {
                check_condition(condition, *line, *col, diagnostics);
                check_body(body, diagnostics);
            }
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }
}

fn check_condition(
    condition: &Expr,
    line: usize,
    column: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(value) = eval_const(condition) else {
        return;
    };

    let always = if truthy(&value) { "true" } else { "false" };
    diagnostics.push(Diagnostic {
        line,
        column,
        message: format!(
            "[warning] Condition is always {always}; it does not depend on any runtime value"
        ),
        rule: RULE,
    });
}

/// Attempts to fold `expr` down to a single constant [`Literal`], returning
/// `None` as soon as any part of it depends on something that can't be
/// known without running the script (an identifier, a call, `Self`/
/// `Parent`, a member/index access, a cast, or a `new` array).
fn eval_const(expr: &Expr) -> Option<Literal> {
    match expr {
        Expr::Literal(literal) => Some(literal.clone()),
        Expr::Unary { op, operand } => eval_unary(*op, &eval_const(operand)?),
        Expr::Binary { left, op, right } => {
            eval_binary(&eval_const(left)?, *op, &eval_const(right)?)
        }
        Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::Call { .. }
        | Expr::Member { .. }
        | Expr::Index { .. }
        | Expr::Cast { .. }
        | Expr::NewArray { .. }
        | Expr::NamedArg { .. } => None,
    }
}

fn truthy(value: &Literal) -> bool {
    match value {
        Literal::Bool(b) => *b,
        Literal::Int { value, .. } => *value != 0,
        Literal::Float(f) => *f != 0.0,
        Literal::String(s) => !s.is_empty(),
        Literal::None => false,
    }
}

fn eval_unary(op: UnaryOp, value: &Literal) -> Option<Literal> {
    match op {
        UnaryOp::Not => Some(Literal::Bool(!truthy(value))),
        UnaryOp::Neg => match value {
            Literal::Int { value, .. } => Some(Literal::int(-value)),
            Literal::Float(f) => Some(Literal::Float(-f)),
            _ => None,
        },
    }
}

/// A numeric literal's value, promoted to `f64` so `Int`/`Float` operands
/// can be combined uniformly; remembers whether either side was a `Float`
/// so arithmetic results can be folded back to the right literal kind.
fn as_number(value: &Literal) -> Option<(f64, bool)> {
    match value {
        Literal::Int { value, .. } => Some((*value as f64, false)),
        Literal::Float(f) => Some((*f, true)),
        _ => None,
    }
}

fn eval_binary(left: &Literal, op: BinaryOp, right: &Literal) -> Option<Literal> {
    match op {
        BinaryOp::And => Some(Literal::Bool(truthy(left) && truthy(right))),
        BinaryOp::Or => Some(Literal::Bool(truthy(left) || truthy(right))),
        BinaryOp::Eq => Some(Literal::Bool(literal_eq(left, right)?)),
        BinaryOp::NotEq => Some(Literal::Bool(!literal_eq(left, right)?)),
        BinaryOp::Add
            if matches!(left, Literal::String(_)) || matches!(right, Literal::String(_)) =>
        {
            match (left, right) {
                (Literal::String(a), Literal::String(b)) => {
                    Some(Literal::String(format!("{a}{b}")))
                }
                _ => None,
            }
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
            let (a, a_float) = as_number(left)?;
            let (b, b_float) = as_number(right)?;
            let result = match op {
                BinaryOp::Add => a + b,
                BinaryOp::Sub => a - b,
                BinaryOp::Mul => a * b,
                BinaryOp::Div => {
                    if b == 0.0 {
                        return None;
                    }
                    a / b
                }
                BinaryOp::Mod => {
                    if b == 0.0 {
                        return None;
                    }
                    a % b
                }
                _ => unreachable!(),
            };
            Some(if a_float || b_float {
                Literal::Float(result)
            } else {
                Literal::int(result as i64)
            })
        }
        BinaryOp::Gt | BinaryOp::Lt | BinaryOp::GtEq | BinaryOp::LtEq => {
            let (a, _) = as_number(left)?;
            let (b, _) = as_number(right)?;
            Some(Literal::Bool(match op {
                BinaryOp::Gt => a > b,
                BinaryOp::Lt => a < b,
                BinaryOp::GtEq => a >= b,
                BinaryOp::LtEq => a <= b,
                _ => unreachable!(),
            }))
        }
    }
}

fn literal_eq(left: &Literal, right: &Literal) -> Option<bool> {
    match (left, right) {
        (Literal::String(a), Literal::String(b)) => Some(a == b),
        (Literal::Bool(a), Literal::Bool(b)) => Some(a == b),
        (Literal::None, Literal::None) => Some(true),
        (Literal::None, _) | (_, Literal::None) => Some(false),
        _ => {
            let (a, _) = as_number(left)?;
            let (b, _) = as_number(right)?;
            Some(a == b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_literal_true_condition() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    If true\n    EndIf\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.contains("always true"));
    }

    #[test]
    fn flags_literal_false_condition() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    If false\n    EndIf\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("always false"));
    }

    #[test]
    fn flags_constant_numeric_comparison() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    If 1 == 1\n    EndIf\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("always true"));

        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    If 1 == 2\n    EndIf\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("always false"));
    }

    #[test]
    fn flags_constant_logical_and_unary_expressions() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If true && false\n    EndIf\n    If !true\n    EndIf\n    If 1 < 2 || 3 > 4\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 3);
        assert!(diagnostics.iter().all(|d| d.message.contains("always")));
    }

    #[test]
    fn folds_every_literal_kind_and_unary_negation() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If 1\n    EndIf\n    If 0.0\n    EndIf\n    If \"text\"\n    EndIf\n    If None\n    EndIf\n    If -1 < 0\n    EndIf\n    If -1.5 < 0.0\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 6);
        let outcomes: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.contains("always true"))
            .collect();
        assert_eq!(outcomes, vec![true, false, true, false, true, true]);
    }

    #[test]
    fn folds_arithmetic_string_and_comparison_operators() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If 1 + 2 == 3\n    EndIf\n    If 5 - 2 == 3\n    EndIf\n    If 2 * 3 == 6\n    EndIf\n    If 8 / 2 == 4\n    EndIf\n    If 7 % 4 == 3\n    EndIf\n    If 1 + 0.5 == 1.5\n    EndIf\n    If \"foo\" + \"bar\" == \"foobar\"\n    EndIf\n    If 2 >= 2\n    EndIf\n    If 2 <= 3\n    EndIf\n    If 2 != 3\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 10);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message.contains("always true")));
    }

    #[test]
    fn literal_equality_handles_bool_none_and_incompatible_values() {
        assert_eq!(
            literal_eq(&Literal::Bool(true), &Literal::Bool(false)),
            Some(false)
        );
        assert_eq!(literal_eq(&Literal::None, &Literal::None), Some(true));
        assert_eq!(literal_eq(&Literal::None, &Literal::int(1)), Some(false));
        assert_eq!(literal_eq(&Literal::int(1), &Literal::None), Some(false));
        assert_eq!(
            literal_eq(&Literal::String("1".into()), &Literal::int(1)),
            None
        );
    }

    #[test]
    fn invalid_constant_operations_are_not_folded() {
        assert_eq!(eval_unary(UnaryOp::Neg, &Literal::Bool(true)), None);
        assert_eq!(as_number(&Literal::String("nope".into())), None);
        assert_eq!(
            eval_binary(
                &Literal::String("left".into()),
                BinaryOp::Add,
                &Literal::int(1),
            ),
            None
        );
        assert_eq!(
            eval_binary(&Literal::int(1), BinaryOp::Div, &Literal::int(0)),
            None
        );
        assert_eq!(
            eval_binary(&Literal::int(1), BinaryOp::Mod, &Literal::int(0)),
            None
        );
    }

    #[test]
    fn flags_constant_while_condition() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    While 1 > 0\n        Return\n    EndWhile\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn does_not_flag_conditions_depending_on_a_runtime_value() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag, Int a)\n    If flag\n    EndIf\n    If a > 0\n    EndIf\n    If a == 1 && flag\n    EndIf\n    If GetValue()\n    EndIf\n    If Self.SomeProperty\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_each_elseif_branch_and_nested_and_state_bodies() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Bool flag)\n        If flag\n        ElseIf true\n            If false\n            EndIf\n        EndIf\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].line, 6);
        assert_eq!(diagnostics[1].line, 7);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }
}
