//! Flags a call to `Utility.Wait`, `RegisterForUpdate`,
//! `RegisterForSingleUpdate`, `RegisterForUpdateGameTime`, or
//! `RegisterForSingleUpdateGameTime` whose interval argument folds to a
//! compile-time-constant number below a configurable minimum (`Config`'s
//! `min_wait_interval`, default `0.1`), since an interval that short runs
//! far more often than is typically useful and can add up to meaningful
//! performance overhead.
//!
//! Like [`crate::division_by_zero`], this only folds an argument built
//! entirely from literals (optionally combined with arithmetic and unary
//! operators); an argument that depends on an identifier, a call, `Self`/
//! `Parent`, a member/index access, a cast, or a `new` array is left
//! unflagged rather than guessed at. Always reported as a `[warning]`,
//! regardless of how far below the minimum the value is.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, UnaryOp};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "short-wait-interval";

pub(crate) struct WaitFunction {
    pub(crate) name: &'static str,
    /// Whether `name` is a native singleton function (only `Utility.Wait`)
    /// always called through its literal script name, rather than an
    /// instance method (the `RegisterFor*` family) reachable unqualified
    /// or through any receiver. See `check` for how this is used.
    global: bool,
}

pub(crate) const WAIT_FUNCTIONS: &[WaitFunction] = &[
    WaitFunction {
        name: "Wait",
        global: true,
    },
    WaitFunction {
        name: "RegisterForUpdate",
        global: false,
    },
    WaitFunction {
        name: "RegisterForSingleUpdate",
        global: false,
    },
    WaitFunction {
        name: "RegisterForUpdateGameTime",
        global: false,
    },
    WaitFunction {
        name: "RegisterForSingleUpdateGameTime",
        global: false,
    },
];

/// Checks every call to `Utility.Wait`/`RegisterForUpdate`/
/// `RegisterForSingleUpdate`/`RegisterForUpdateGameTime`/
/// `RegisterForSingleUpdateGameTime` in `source`, flagging one whose sole
/// argument folds to a constant number below `minimum`.
pub fn check(source: &str, minimum: f64) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        check_body(&function.body, minimum, &mut diagnostics);
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

fn check_body(body: &[Stmt], minimum: f64, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    walk_expr(value, minimum, diagnostics);
                }
            }
            Stmt::Assign { target, value, .. } => {
                walk_expr(target, minimum, diagnostics);
                walk_expr(value, minimum, diagnostics);
            }
            Stmt::Expr { value, .. } => walk_expr(value, minimum, diagnostics),
            Stmt::Return {
                value: Some(value), ..
            } => {
                walk_expr(value, minimum, diagnostics);
            }
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch {
                    condition, body, ..
                } in branches
                {
                    walk_expr(condition, minimum, diagnostics);
                    check_body(body, minimum, diagnostics);
                }
                check_body(else_body, minimum, diagnostics);
            }
            Stmt::While {
                condition, body, ..
            } => {
                walk_expr(condition, minimum, diagnostics);
                check_body(body, minimum, diagnostics);
            }
        }
    }
}

fn walk_expr(expr: &Expr, minimum: f64, diagnostics: &mut Vec<Diagnostic>) {
    if let Expr::Call {
        callee,
        args,
        line,
        col,
    } = expr
    {
        if let Some(function) = matching_function(callee) {
            if let Some(argument) = args.first() {
                let value_expr = match argument {
                    Expr::NamedArg { value, .. } => value,
                    other => other,
                };
                if let Some(value) = eval_const(value_expr) {
                    if let Some((number, _)) = as_number(&value) {
                        if number < minimum {
                            diagnostics.push(Diagnostic {
                                line: *line,
                                column: *col,
                                message: format!(
                                    "[warning] {}({number}) is below the configured minimum \
                                     interval of {minimum}; an interval that short runs far \
                                     more often than typically useful and can add up to \
                                     meaningful performance overhead",
                                    function.name
                                ),
                                rule: RULE,
                            });
                        }
                    }
                }
                walk_expr(value_expr, minimum, diagnostics);
            }
            for arg in args.iter().skip(1) {
                walk_expr(arg, minimum, diagnostics);
            }
            walk_expr(callee, minimum, diagnostics);
            return;
        }
        walk_expr(callee, minimum, diagnostics);
        for arg in args {
            walk_expr(arg, minimum, diagnostics);
        }
        return;
    }

    match expr {
        Expr::Binary { left, right, .. } => {
            walk_expr(left, minimum, diagnostics);
            walk_expr(right, minimum, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, minimum, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, minimum, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, minimum, diagnostics);
            walk_expr(index, minimum, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, minimum, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, minimum, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, minimum, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent | Expr::Call { .. } => {
        }
    }
}

/// Whether `callee` is a call to one of [`WAIT_FUNCTIONS`], honoring each
/// rule's `global` flag the same way `forbidden_functions`/`slow_functions`
/// do: a `global` rule (`Utility.Wait`) only matches when explicitly
/// qualified by that literal script name, while a non-`global` rule (the
/// `RegisterFor*` family) matches unqualified or through any receiver.
///
/// Also used by [`crate::magic_numbers`] to exempt these same calls'
/// interval arguments from its "loose" mode.
pub(crate) fn matching_function(callee: &Expr) -> Option<&'static WaitFunction> {
    match callee {
        Expr::Identifier(name) => WAIT_FUNCTIONS
            .iter()
            .find(|function| !function.global && function.name.eq_ignore_ascii_case(name)),
        Expr::Member { object, property } => {
            let function = WAIT_FUNCTIONS
                .iter()
                .find(|function| function.name.eq_ignore_ascii_case(property))?;
            if function.global {
                let Expr::Identifier(qualifier) = object.as_ref() else {
                    return None;
                };
                if !qualifier.eq_ignore_ascii_case("Utility") {
                    return None;
                }
            }
            Some(function)
        }
        _ => None,
    }
}

/// Attempts to fold `expr` down to a single constant numeric [`Literal`],
/// the same way [`crate::division_by_zero`] does: returning `None` as soon
/// as any part of it depends on something that can't be known without
/// running the script.
fn eval_const(expr: &Expr) -> Option<Literal> {
    match expr {
        Expr::Literal(literal @ (Literal::Int(_) | Literal::Float(_))) => Some(literal.clone()),
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => match eval_const(operand)? {
            Literal::Int(i) => Some(Literal::Int(-i)),
            Literal::Float(f) => Some(Literal::Float(-f)),
            _ => None,
        },
        Expr::Binary {
            left,
            op: op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul),
            right,
        } => {
            let (a, a_float) = as_number(&eval_const(left)?)?;
            let (b, b_float) = as_number(&eval_const(right)?)?;
            let result = match op {
                BinaryOp::Add => a + b,
                BinaryOp::Sub => a - b,
                BinaryOp::Mul => a * b,
                _ => unreachable!(),
            };
            Some(if a_float || b_float {
                Literal::Float(result)
            } else {
                Literal::Int(result as i64)
            })
        }
        _ => None,
    }
}

/// Returns a folded literal's numeric value, alongside whether it was a
/// `Float` (as opposed to an `Int`) literal.
fn as_number(value: &Literal) -> Option<(f64, bool)> {
    match value {
        Literal::Int(i) => Some((*i as f64, false)),
        Literal::Float(f) => Some((*f, true)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_wait_below_the_default_minimum() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.05)\nEndFunction\n",
            0.1,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("Wait(0.05)"));
    }

    #[test]
    fn does_not_flag_wait_at_or_above_the_minimum() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.1)\n    Utility.Wait(1.0)\nEndFunction\n",
            0.1,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_unqualified_wait() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Wait(0.01)\nEndFunction\n",
            0.1,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_wait_on_an_unrelated_script() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(MyScript akOther)\n    akOther.Wait(0.01)\nEndFunction\n",
            0.1,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_unqualified_register_for_update() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    RegisterForSingleUpdate(0.02)\nEndFunction\n",
            0.1,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("RegisterForSingleUpdate(0.02)"));
    }

    #[test]
    fn flags_register_for_update_on_an_arbitrary_receiver() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    akRef.RegisterForUpdate(0.05)\nEndFunction\n",
            0.1,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("RegisterForUpdate"));
    }

    #[test]
    fn flags_register_for_update_game_time_variants() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    RegisterForUpdateGameTime(0.05)\n    RegisterForSingleUpdateGameTime(0.05)\nEndFunction\n",
            0.1,
        );

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn does_not_flag_register_for_update_at_or_above_the_minimum() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    RegisterForSingleUpdate(0.1)\nEndFunction\n",
            0.1,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn honors_a_configured_minimum() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.4)\nEndFunction\n",
            0.5,
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_constant_expression_that_folds_below_the_minimum() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.2 - 0.15)\nEndFunction\n",
            0.1,
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_named_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(afSeconds = 0.01)\nEndFunction\n",
            0.1,
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_a_runtime_value() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Float afSeconds)\n    Utility.Wait(afSeconds)\n    Utility.Wait(GetInterval())\nEndFunction\n",
            0.1,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_conditions_return_values_and_nested_state_bodies() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        If true\n            Utility.Wait(0.01)\n        EndIf\n    EndFunction\nEndState\n",
            0.1,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n", 0.1).is_empty());
    }
}
