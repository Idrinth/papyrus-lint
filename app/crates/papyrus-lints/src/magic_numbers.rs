//! Flags a numeric literal used directly in an expression rather than
//! through a named constant, property, or local variable, as a
//! `[warning]`. Disabled by default (see [`crate::config::Rules::magic_numbers`]):
//! many existing scripts contain plenty of unremarkable literal numbers,
//! so a project has to opt in explicitly.
//!
//! `-1`, `0`, and `1` are never flagged, since they're near-universally
//! used directly (array bounds, increments, sentinel/empty checks) without
//! losing any clarity from being spelled out.
//!
//! A literal that's the entire value given to a declaration or assignment
//! (`Int kMaxTargets = 5`, later reassigned as `kMaxTargets = 6`) is left
//! alone too: naming it there already gives it the meaning this lint is
//! after. Only the bare literal itself is exempted this way; a literal
//! nested inside a more complex initializer (`Int kMaxTargets = 5 + 1`) is
//! still checked, since the declaration's name doesn't explain what either
//! operand means on its own.
//!
//! By default (the "loose" [`MagicNumbers`] mode), a numeric literal passed
//! as an argument to `Utility.Wait`, `RegisterForUpdate`,
//! `RegisterForSingleUpdate`, `RegisterForUpdateGameTime`, or
//! `RegisterForSingleUpdateGameTime` (see [`crate::short_wait_interval`],
//! which matches the same calls) is left unflagged too, since a hardcoded
//! interval there is both common and usually self-explanatory. The
//! "strict" mode also checks those arguments like any other.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, UnaryOp};
use serde::{Deserialize, Serialize};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "magic-numbers";

/// Whether this lint also checks the interval argument of a
/// `Utility.Wait`/`RegisterForUpdate`/`RegisterForSingleUpdate`/
/// `RegisterForUpdateGameTime`/`RegisterForSingleUpdateGameTime` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagicNumbers {
    /// The interval argument of a `Utility.Wait`/`RegisterFor*` call is
    /// never flagged.
    #[default]
    Loose,
    /// Every numeric literal is checked, including a `Utility.Wait`/
    /// `RegisterFor*` call's interval argument.
    Strict,
}

/// Checks `source` for numeric literals used directly rather than through
/// a named constant, property, or local variable, per `mode`.
pub fn check(source: &str, mode: MagicNumbers) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for variable in &script.variables {
        if let Some(value) = &variable.value {
            walk_declaration_value(value, mode, variable.line, &mut diagnostics);
        }
    }
    for function in all_functions(&script) {
        check_body(&function.body, mode, &mut diagnostics);
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

fn check_body(body: &[Stmt], mode: MagicNumbers, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    walk_declaration_value(value, mode, decl.line, diagnostics);
                }
            }
            Stmt::Assign {
                target,
                value,
                line,
                ..
            } => {
                walk_expr(target, mode, *line, false, diagnostics);
                walk_declaration_value(value, mode, *line, diagnostics);
            }
            Stmt::Expr { value, line } => walk_expr(value, mode, *line, false, diagnostics),
            Stmt::Return {
                value: Some(value),
                line,
            } => {
                walk_expr(value, mode, *line, false, diagnostics);
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
                    walk_expr(condition, mode, *line, false, diagnostics);
                    check_body(body, mode, diagnostics);
                }
                check_body(else_body, mode, diagnostics);
            }
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                walk_expr(condition, mode, *line, false, diagnostics);
                check_body(body, mode, diagnostics);
            }
        }
    }
}

/// Checks a declaration's or assignment's value expression: a bare numeric
/// literal (optionally negated) is exempt, since naming it there already
/// gives it the meaning this lint is after; anything else is walked
/// normally, so a literal nested inside a more complex initializer is
/// still checked.
fn walk_declaration_value(
    value: &Expr,
    mode: MagicNumbers,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if is_bare_number_literal(value) {
        return;
    }
    walk_expr(value, mode, line, false, diagnostics);
}

fn is_bare_number_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(Literal::Int { .. } | Literal::Float(_)) => true,
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => matches!(
            operand.as_ref(),
            Expr::Literal(Literal::Int { .. } | Literal::Float(_))
        ),
        _ => false,
    }
}

/// Recursively walks `expr`, flagging every numeric literal it finds
/// (subject to the ignored-value list and, in "loose" mode, the
/// `Wait`/`RegisterFor*` exemption). `wait_exempt` is threaded through
/// arithmetic composition (`Binary`, `Unary`) so a literal combined with
/// others inside an exempted call's argument stays exempt too, but resets
/// to `false` across a `Call`, `Member`, `Index`, `Cast`, or `NewArray`
/// boundary, since those introduce a value of their own rather than
/// composing the exempted argument's.
fn walk_expr(
    expr: &Expr,
    mode: MagicNumbers,
    line: usize,
    wait_exempt: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Literal(Literal::Int { value, .. }) => {
            if !wait_exempt {
                flag_int(*value, line, diagnostics);
            }
        }
        Expr::Literal(Literal::Float(f)) => {
            if !wait_exempt {
                flag_float(*f, line, diagnostics);
            }
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => match operand.as_ref() {
            Expr::Literal(Literal::Int { value, .. }) => {
                if !wait_exempt {
                    flag_int(-*value, line, diagnostics);
                }
            }
            Expr::Literal(Literal::Float(f)) => {
                if !wait_exempt {
                    flag_float(-*f, line, diagnostics);
                }
            }
            _ => walk_expr(operand, mode, line, wait_exempt, diagnostics),
        },
        Expr::Unary { operand, .. } => walk_expr(operand, mode, line, wait_exempt, diagnostics),
        Expr::Binary { left, right, .. } => {
            walk_expr(left, mode, line, wait_exempt, diagnostics);
            walk_expr(right, mode, line, wait_exempt, diagnostics);
        }
        Expr::Call { callee, args, .. } => {
            let exempt = mode == MagicNumbers::Loose
                && crate::short_wait_interval::matching_function(callee).is_some();
            for arg in args {
                let value_expr = match arg {
                    Expr::NamedArg { value, .. } => value.as_ref(),
                    other => other,
                };
                walk_expr(value_expr, mode, line, exempt, diagnostics);
            }
            walk_expr(callee, mode, line, false, diagnostics);
        }
        Expr::NamedArg { value, .. } => walk_expr(value, mode, line, wait_exempt, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, mode, line, false, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, mode, line, false, diagnostics);
            walk_expr(index, mode, line, false, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, mode, line, false, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, mode, line, false, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

fn is_ignored_int(value: i64) -> bool {
    matches!(value, -1..=1)
}

fn is_ignored_float(value: f64) -> bool {
    value == -1.0 || value == 0.0 || value == 1.0
}

fn flag_int(value: i64, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    if !is_ignored_int(value) {
        push(value.to_string(), line, diagnostics);
    }
}

fn flag_float(value: f64, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    if !is_ignored_float(value) {
        push(value.to_string(), line, diagnostics);
    }
}

fn push(display: String, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic {
        line,
        column: 1,
        message: format!(
            "[warning] Magic number {display}; extract it into a named constant, property, \
             or local variable so its meaning is clear"
        ),
        rule: RULE,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_literal_used_directly_in_a_call_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    DoThing(42)\nEndFunction\n",
            MagicNumbers::Loose,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("42"));
    }

    #[test]
    fn does_not_flag_ignored_values() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    DoThing(0)\n    DoThing(1)\n    DoThing(-1)\nEndFunction\n",
            MagicNumbers::Loose,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_ignored_float_values() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    DoThing(0.0)\n    DoThing(1.0)\n    DoThing(-1.0)\nEndFunction\n",
            MagicNumbers::Loose,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_non_ignored_negative_literal() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    DoThing(-5)\nEndFunction\n",
            MagicNumbers::Loose,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("-5"));
    }

    #[test]
    fn does_not_flag_a_bare_literal_local_variable_declaration() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int kMaxTargets = 5\nEndFunction\n",
            MagicNumbers::Loose,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_bare_literal_reassignment() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int kMaxTargets = 5\n    kMaxTargets = 6\nEndFunction\n",
            MagicNumbers::Loose,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_literal_nested_in_a_declaration_initializer() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int kMaxTargets = 5 + 1\nEndFunction\n",
            MagicNumbers::Loose,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains('5'));
    }

    #[test]
    fn does_not_flag_a_bare_literal_script_variable_declaration() {
        let diagnostics = check(
            "ScriptName Example\n\nInt kMaxTargets = 5\n\nFunction Test()\nEndFunction\n",
            MagicNumbers::Loose,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_property_defaults() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property MaxTargets = 5 Auto\n",
            MagicNumbers::Loose,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn loose_mode_does_not_flag_wait_or_register_for_update_intervals() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(5)\n    RegisterForSingleUpdate(5)\nEndFunction\n",
            MagicNumbers::Loose,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn loose_mode_exemption_does_not_reach_through_a_nested_call() {
        // The exemption covers Wait's own interval argument, not a call
        // nested inside it.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(SomeCall(5))\nEndFunction\n",
            MagicNumbers::Loose,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains('5'));
    }

    #[test]
    fn loose_mode_exemption_covers_an_arithmetic_interval_expression() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(5 + 2)\nEndFunction\n",
            MagicNumbers::Loose,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn strict_mode_flags_wait_and_register_for_update_intervals() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(5)\n    RegisterForSingleUpdate(5)\nEndFunction\n",
            MagicNumbers::Strict,
        );

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn checks_conditions_and_nested_state_bodies() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        If GetValue() > 42\n            DoThing()\n        EndIf\n    EndFunction\nEndState\n",
            MagicNumbers::Loose,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check(
            "ScriptName Example\n\nFunction Test(\nEndFunction\n",
            MagicNumbers::Loose
        )
        .is_empty());
    }
}
