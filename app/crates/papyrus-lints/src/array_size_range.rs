//! Flags, as an `[error]`, a `new <Type>[<N>]` array creation whose literal
//! `N` falls outside the range Papyrus allows for a script-created array
//! (`0` to `128`), since a size outside that range is almost never
//! intended: per the CreationKit wiki's [Arrays
//! (Papyrus)](https://ck.uesp.net/wiki/Arrays_(Papyrus)) page, an array
//! created by a script via `New` (or grown with `Add()`) is hard-capped at
//! 128 elements by the engine, and a negative size makes no sense at all.
//! That cap does not apply to an array returned by a native function or to
//! an editor-populated array `Property`, since neither is created this way.
//!
//! This is a deliberately small, standalone check split out from
//! [`crate::array_bounds`]: unlike that lint's flow-sensitive tracking of
//! which local variable currently holds an array of which size, this one
//! only ever looks at a `new <Type>[<N>]` expression's own literal size, so
//! it has no state to track and nothing else can disable or narrow it.

use papyrus_parser::ast::{BinaryOp, Expr, IfBranch, Literal, Stmt, UnaryOp};

use crate::none_form_usage::all_functions;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "array-size-range";

/// The maximum number of elements an array created by a script (via `New`
/// or grown with `Add()`) can hold, per the CreationKit wiki's [Arrays
/// (Papyrus)](https://ck.uesp.net/wiki/Arrays_(Papyrus)) page.
const MAX_NEW_ARRAY_SIZE: i64 = 128;

/// Checks every function/event in `source` for a `new <Type>[<N>]` whose
/// literal `N` falls outside `0..=128`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        walk_body(&function.body, &mut diagnostics);
    }
    diagnostics
}

fn walk_body(body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    check_expr(value, diagnostics, decl.line);
                }
            }
            Stmt::Assign {
                target,
                value,
                line,
                ..
            } => {
                check_expr(target, diagnostics, *line);
                check_expr(value, diagnostics, *line);
            }
            Stmt::Expr { value, line } => check_expr(value, diagnostics, *line),
            Stmt::Return {
                value: Some(value),
                line,
            } => check_expr(value, diagnostics, *line),
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => handle_if(branches, else_body, diagnostics),
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                check_expr(condition, diagnostics, *line);
                walk_body(body, diagnostics);
            }
        }
    }
}

fn handle_if(branches: &[IfBranch], else_body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    for branch in branches {
        check_expr(&branch.condition, diagnostics, branch.line);
        walk_body(&branch.body, diagnostics);
    }
    walk_body(else_body, diagnostics);
}

/// Recursively checks `expr` for a `new <Type>[<N>]` whose literal `N`
/// falls outside `0..=128`, recursing into every sub-expression so a
/// `new` nested anywhere within a larger expression (an argument, an
/// index, ...) is still found.
fn check_expr(expr: &Expr, diagnostics: &mut Vec<Diagnostic>, line: usize) {
    match expr {
        Expr::Index { object, index } => {
            check_expr(object, diagnostics, line);
            check_expr(index, diagnostics, line);
        }
        Expr::Member { object, .. } => check_expr(object, diagnostics, line),
        Expr::Call { callee, args, .. } => {
            check_expr(callee, diagnostics, line);
            for arg in args {
                check_expr(arg, diagnostics, line);
            }
        }
        Expr::Binary { left, right, .. } => {
            check_expr(left, diagnostics, line);
            check_expr(right, diagnostics, line);
        }
        Expr::Unary { operand, .. } => check_expr(operand, diagnostics, line),
        Expr::Cast { value, .. } => check_expr(value, diagnostics, line),
        Expr::NewArray { size, .. } => {
            check_expr(size, diagnostics, line);
            if let Some(literal_size) = eval_const_int(size) {
                if !(0..=MAX_NEW_ARRAY_SIZE).contains(&literal_size) {
                    diagnostics.push(Diagnostic {
                        line,
                        column: 1,
                        message: format!(
                            "[error] Array size {literal_size} is outside the range Papyrus \
                             allows (0 to {MAX_NEW_ARRAY_SIZE}) for an array created with `new`"
                        ),
                        rule: RULE,
                    });
                }
            }
        }
        Expr::NamedArg { value, .. } => check_expr(value, diagnostics, line),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Attempts to fold `expr` down to a single constant `Int`, returning `None`
/// as soon as any part of it depends on something that can't be known
/// without running the script (an identifier, a call, `Self`/`Parent`, a
/// member/index access, a cast, a `new` array, a `Float`, division, or
/// modulo).
fn eval_const_int(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Literal(Literal::Int { value, .. }) => Some(*value),
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => eval_const_int(operand).map(|value| -value),
        Expr::Binary {
            left,
            op: op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul),
            right,
        } => {
            let left = eval_const_int(left)?;
            let right = eval_const_int(right)?;
            Some(match op {
                BinaryOp::Add => left + right,
                BinaryOp::Sub => left - right,
                BinaryOp::Mul => left * right,
                _ => unreachable!(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_new_array_larger_than_the_engine_maximum() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[200]\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[error]"));
        assert!(diagnostics[0].message.contains("200"));
        assert!(diagnostics[0].message.contains("128"));
    }

    #[test]
    fn flags_a_new_array_with_a_negative_size() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[-1]\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn does_not_flag_a_new_array_at_the_engine_maximum() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[128]\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_new_array_at_zero() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[0]\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_new_array_whose_size_is_not_a_literal() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int n)\n    Int[] a = new Int[n]\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn constant_folding_handles_literal_arithmetic_in_the_size() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[100 + 50]\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn checks_a_new_array_nested_in_a_call_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Consume(new Int[200])\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Int[] a = new Int[200]\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn checks_conditions_and_branches_of_an_if() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n        Int[] a = new Int[200]\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn checks_a_while_loop_body() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    While flag\n        Int[] a = new Int[200]\n        flag = false\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }
}
