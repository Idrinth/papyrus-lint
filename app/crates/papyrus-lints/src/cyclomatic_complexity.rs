//! Flags functions/events whose cyclomatic complexity exceeds a configured
//! threshold.
//!
//! Complexity starts at 1 for the function itself and gains 1 for every
//! extra path through it: each `If`/`ElseIf` branch, each `While` loop, and
//! each short-circuiting `&&`/`||` operator (which itself introduces a
//! branch, since the right-hand side may or may not be evaluated). This
//! works from the parsed AST rather than raw tokens, since it needs the
//! block structure of the function body; a script that doesn't parse
//! cleanly is left unchecked rather than guessed at.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "cyclomatic-complexity";

/// Checks `source` for functions/events whose cyclomatic complexity exceeds
/// `warning` or `error`. A function at or below `warning` is not flagged; one
/// above `warning` but at or below `error` is flagged as `[warning]`; one
/// above `error` is flagged as `[error]`.
pub fn check(source: &str, warning: usize, error: usize) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    all_functions(&script)
        .filter_map(|function| {
            let complexity = complexity_of(function);
            let level = if complexity > error {
                "error"
            } else if complexity > warning {
                "warning"
            } else {
                return None;
            };

            Some(Diagnostic {
                line: function.line,
                column: 1,
                message: format!(
                    "[{}] Function '{}' has a cyclomatic complexity of {} (warning: {}, error: {})",
                    level, function.name, complexity, warning, error
                ),
                rule: RULE,
            })
        })
        .collect()
}

/// Iterates every function declared directly on a script, plus every
/// function declared in each of its states.
fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

fn complexity_of(function: &FunctionDecl) -> usize {
    1 + function.body.iter().map(stmt_complexity).sum::<usize>()
}

fn stmt_complexity(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::VarDecl(decl) => decl.value.as_ref().map_or(0, expr_complexity),
        Stmt::Assign { target, value, .. } => expr_complexity(target) + expr_complexity(value),
        Stmt::Expr { value, .. } => expr_complexity(value),
        Stmt::Return { value, .. } => value.as_ref().map_or(0, expr_complexity),
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            branches
                .iter()
                .map(|branch| {
                    1 + expr_complexity(&branch.condition) + body_complexity(&branch.body)
                })
                .sum::<usize>()
                + body_complexity(else_body)
        }
        Stmt::While {
            condition, body, ..
        } => 1 + expr_complexity(condition) + body_complexity(body),
    }
}

fn body_complexity(body: &[Stmt]) -> usize {
    body.iter().map(stmt_complexity).sum()
}

fn expr_complexity(expr: &Expr) -> usize {
    match expr {
        Expr::Binary { left, op, right } => {
            let branch = matches!(op, BinaryOp::And | BinaryOp::Or) as usize;
            branch + expr_complexity(left) + expr_complexity(right)
        }
        Expr::Unary { operand, .. } => expr_complexity(operand),
        Expr::Call { callee, args, .. } => {
            expr_complexity(callee) + args.iter().map(expr_complexity).sum::<usize>()
        }
        Expr::Member { object, .. } => expr_complexity(object),
        Expr::Index { object, index } => expr_complexity(object) + expr_complexity(index),
        Expr::Cast { value, .. } => expr_complexity(value),
        Expr::NewArray { size, .. } => expr_complexity(size),
        Expr::NamedArg { value, .. } => expr_complexity(value),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_function_has_baseline_complexity_of_one() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n",
            0,
            20,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("complexity of 1"));
    }

    #[test]
    fn does_not_flag_functions_at_or_below_warning_threshold() {
        let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    EndIf\nEndFunction\n";

        // Complexity is 2 (baseline 1 + one If branch); default warning is 10.
        assert!(check(source, 10, 20).is_empty());
    }

    #[test]
    fn flags_warning_level_between_thresholds() {
        let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    EndIf\nEndFunction\n";

        let diagnostics = check(source, 1, 20);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("complexity of 2"));
        assert_eq!(diagnostics[0].line, 3);
    }

    #[test]
    fn flags_error_level_above_error_threshold() {
        let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    EndIf\nEndFunction\n";

        let diagnostics = check(source, 0, 1);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.starts_with("[error]"));
    }

    #[test]
    fn counts_elseif_and_else_if_branches_and_while_loops() {
        let source = "ScriptName Example\n\nFunction Test()\n    If a\n        Int i = 1\n    ElseIf b\n        Int i = 2\n    Else\n        Int i = 3\n    EndIf\n    While c\n        Int i = 4\n    EndWhile\nEndFunction\n";

        // Baseline 1 + If branch + ElseIf branch + While = 4.
        let diagnostics = check(source, 3, 20);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("complexity of 4"));
    }

    #[test]
    fn counts_short_circuit_logical_operators_in_conditions() {
        let source =
            "ScriptName Example\n\nFunction Test()\n    If a && b || c\n        Int i = 1\n    EndIf\nEndFunction\n";

        // Baseline 1 + If branch + && + || = 4.
        let diagnostics = check(source, 3, 20);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("complexity of 4"));
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let source = "ScriptName Example\n\nState Active\n    Function Test()\n        If a\n            Int i = 1\n        EndIf\n    EndFunction\nEndState\n";

        let diagnostics = check(source, 1, 20);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Test"));
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check(
            "ScriptName Example\n\nFunction Test(\nEndFunction\n",
            10,
            20
        )
        .is_empty());
    }

    #[test]
    fn counts_logical_operators_in_each_statement_expression() {
        let source = "ScriptName Example\n\nBool Function Test(Bool a, Bool b)\n    Bool value = a && b\n    value = a || b\n    Consume(a && b)\n    Return a || b\nEndFunction\n";

        // Baseline 1 plus one short-circuit operator in each of the four
        // statement expression forms above.
        let diagnostics = check(source, 4, 20);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("complexity of 5"));
    }

    #[test]
    fn counts_nested_control_flow_inside_else_bodies() {
        let source = "ScriptName Example\n\nFunction Test()\n    If ready\n        Return\n    Else\n        While waiting\n            If failed\n                Return\n            EndIf\n        EndWhile\n    EndIf\nEndFunction\n";

        // Else itself adds no path, but its nested While and If do.
        let diagnostics = check(source, 3, 20);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("complexity of 4"));
    }

    #[test]
    fn reports_each_over_threshold_function_independently() {
        let source = "ScriptName Example\n\nFunction First()\n    If ready\n    EndIf\nEndFunction\n\nEvent OnInit()\n    While waiting\n    EndWhile\nEndEvent\n";

        let diagnostics = check(source, 1, 20);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.line, diagnostic.column))
                .collect::<Vec<_>>(),
            vec![(3, 1), (8, 1)]
        );
        assert!(diagnostics[0].message.contains("'First'"));
        assert!(diagnostics[1].message.contains("'OnInit'"));
        assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule == RULE));
    }
}
