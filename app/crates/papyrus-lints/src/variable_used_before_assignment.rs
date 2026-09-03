//! Flags a local variable, declared without an initial value (`Int i`
//! rather than `Int i = 0`), that's read before anything in the function
//! ever assigns it a value — so the read actually observes Papyrus's
//! implicit per-type default (`0`, `0.0`, `False`, `""`, or `None`) rather
//! than a value the author chose, which is usually an oversight rather
//! than something intended.
//!
//! This works from the parsed AST, tracking which declared-without-a-value
//! locals are still unassigned as it walks each function body in order,
//! the same flow-sensitive shape [`crate::none_form_usage`] uses for
//! tracking known-`None` variables: a variable starts out unassigned at
//! its declaration and stops being tracked the moment a plain `name =
//! value` assignment reaches it (a compound assignment like `name += 1`
//! reads the still-unassigned value first, so it's flagged too, before the
//! variable then counts as assigned from that point on). `If`/`ElseIf`/
//! `Else` branches are each walked from the same incoming state, and a
//! branch that unconditionally `Return`s doesn't contribute its exit state
//! to what follows the `If` — a variable stays flagged as possibly
//! unassigned afterward if it's still unassigned along any surviving
//! path. A `While` loop may run zero times, so an assignment made only
//! inside its body is never assumed to have run by the time execution
//! reaches the code after the loop. Function parameters and script
//! properties always have a value by the time a function runs and are
//! never tracked by this lint.
//!
//! An `==`/`!=` comparison against the type's own implicit default
//! (`None`, `0`, `0.0`, `False`, or `""`) is a deliberate gate on "has this
//! been set yet?" rather than a genuine read of the value, so the compared
//! variable is never flagged for that comparison specifically (other reads
//! of it elsewhere still are).

use std::collections::HashSet;

use papyrus_parser::ast::{AssignOp, BinaryOp, Expr, IfBranch, Literal, Stmt};

use crate::none_form_usage::{all_functions, diverges};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "variable-used-before-assignment";

/// Checks every function/event in `source` for a local variable read before
/// it's ever been assigned a value.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        let mut unassigned = HashSet::new();
        walk_body(&function.body, &mut unassigned, &mut diagnostics);
    }
    diagnostics
}

fn walk_body(body: &[Stmt], unassigned: &mut HashSet<String>, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    check_expr(value, unassigned, diagnostics, decl.line);
                } else {
                    unassigned.insert(decl.name.to_lowercase());
                }
            }
            Stmt::Assign {
                target,
                op,
                value,
                line,
            } => {
                check_expr(value, unassigned, diagnostics, *line);
                match (target, op) {
                    (Expr::Identifier(name), AssignOp::Assign) => {
                        unassigned.remove(&name.to_lowercase());
                    }
                    (Expr::Identifier(name), _) => {
                        // A compound assignment (`x += 1`, ...) reads the
                        // current value of x before writing the new one.
                        check_identifier(name, unassigned, diagnostics, *line);
                        unassigned.remove(&name.to_lowercase());
                    }
                    _ => check_expr(target, unassigned, diagnostics, *line),
                }
            }
            Stmt::Expr { value, line } => check_expr(value, unassigned, diagnostics, *line),
            Stmt::Return {
                value: Some(value),
                line,
            } => check_expr(value, unassigned, diagnostics, *line),
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => handle_if(branches, else_body, unassigned, diagnostics),
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                check_expr(condition, unassigned, diagnostics, *line);
                let mut loop_vars = unassigned.clone();
                walk_body(body, &mut loop_vars, diagnostics);
                // A `While` loop may run zero times, so an assignment made
                // only inside its body can't be assumed to have happened
                // by the time execution reaches the code after the loop.
            }
        }
    }
}

/// Handles an `If`/`ElseIf`/`Else` chain: each branch (and the trailing
/// `Else`, if any) is checked from the same incoming state, and only
/// branches that don't unconditionally `Return` contribute their exit
/// state to what follows the `If` — a variable stays flagged as possibly
/// unassigned afterward if it's still unassigned along any surviving path.
fn handle_if(
    branches: &[IfBranch],
    else_body: &[Stmt],
    unassigned: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_vars = unassigned.clone();
    let mut surviving = Vec::new();

    for branch in branches {
        check_expr(&branch.condition, &entry_vars, diagnostics, branch.line);
        let mut branch_vars = entry_vars.clone();
        walk_body(&branch.body, &mut branch_vars, diagnostics);
        if !diverges(&branch.body) {
            surviving.push(branch_vars);
        }
    }

    let mut else_vars = entry_vars.clone();
    walk_body(else_body, &mut else_vars, diagnostics);
    if !diverges(else_body) {
        surviving.push(else_vars);
    }

    *unassigned = if surviving.is_empty() {
        // Every branch (including the implicit/explicit else) returns, so
        // nothing after the `If` is reached through it; keep the pre-`If`
        // state rather than guess.
        entry_vars
    } else {
        surviving.into_iter().flatten().collect()
    };
}

/// Flags `name` if it's currently tracked as unassigned in `unassigned`.
fn check_identifier(
    name: &str,
    unassigned: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    line: usize,
) {
    if unassigned.contains(&name.to_lowercase()) {
        diagnostics.push(Diagnostic {
            line,
            column: 1,
            message: format!(
                "[warning] Local variable '{name}' is used here before it's ever assigned a value; it still holds its default"
            ),
            rule: RULE,
        });
    }
}

/// Recursively checks `expr` for a read of a variable currently tracked as
/// unassigned in `unassigned`.
fn check_expr(
    expr: &Expr,
    unassigned: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    line: usize,
) {
    match expr {
        Expr::Identifier(name) => check_identifier(name, unassigned, diagnostics, line),
        Expr::Member { object, .. } => check_expr(object, unassigned, diagnostics, line),
        Expr::Call { callee, args, .. } => {
            check_expr(callee, unassigned, diagnostics, line);
            for arg in args {
                check_expr(arg, unassigned, diagnostics, line);
            }
        }
        Expr::Binary { left, op, right } => {
            let is_default_value_gate =
                matches!(op, BinaryOp::Eq | BinaryOp::NotEq) && default_value_gate(left, right);
            if !is_default_value_gate {
                check_expr(left, unassigned, diagnostics, line);
                check_expr(right, unassigned, diagnostics, line);
            }
        }
        Expr::Unary { operand, .. } => check_expr(operand, unassigned, diagnostics, line),
        Expr::Index { object, index } => {
            check_expr(object, unassigned, diagnostics, line);
            check_expr(index, unassigned, diagnostics, line);
        }
        Expr::Cast { value, .. } => check_expr(value, unassigned, diagnostics, line),
        Expr::NewArray { size, .. } => check_expr(size, unassigned, diagnostics, line),
        Expr::NamedArg { value, .. } => check_expr(value, unassigned, diagnostics, line),
        Expr::Literal(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Whether `left`/`right` (in either order) is a plain identifier compared
/// against a literal spelling of its type's implicit default (`None`, `0`,
/// `0.0`, `False`, or `""`) — the "has this been set yet?" gate pattern this
/// lint deliberately doesn't treat as a use of the variable.
fn default_value_gate(left: &Expr, right: &Expr) -> bool {
    matches!(
        (left, right),
        (Expr::Identifier(_), Expr::Literal(lit)) | (Expr::Literal(lit), Expr::Identifier(_))
            if is_default_value_literal(lit)
    )
}

/// Whether `literal` is the implicit default value for some Papyrus type.
fn is_default_value_literal(literal: &Literal) -> bool {
    match literal {
        Literal::None | Literal::Int { value: 0, .. } | Literal::Bool(false) => true,
        Literal::Float(value) => *value == 0.0,
        Literal::String(value) => value.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_variable_read_before_any_assignment() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("'i'"));
    }

    #[test]
    fn does_not_flag_variable_declared_with_an_initial_value() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 0\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_variable_assigned_before_use() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    i = 1\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_compound_assignment_reading_an_unassigned_variable() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Int i\n    i += 1\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_flag_after_compound_assignment_establishes_a_value() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    i += 1\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn flags_read_inside_the_initializer_of_another_declaration() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Int i\n    Int j = i\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert!(diagnostics[0].message.contains("'i'"));
    }

    #[test]
    fn flags_read_through_a_member_access() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a\n    a.GetName()\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_flag_after_both_if_branches_assign() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    If flag\n        i = 1\n    Else\n        i = 2\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_use_still_possibly_unassigned_after_one_sided_assignment() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    If flag\n        i = 1\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 8);
    }

    #[test]
    fn does_not_flag_after_every_branch_returns_or_assigns() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    If flag\n        Return\n    Else\n        i = 2\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_after_a_guard_clause_assigns_before_falling_through() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    If flag\n        i = 1\n    Else\n        Return\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_use_after_while_loop_since_it_may_run_zero_times() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    While flag\n        i = 1\n        flag = false\n    EndWhile\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 9);
    }

    #[test]
    fn does_not_flag_function_parameters() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test(Int count)\n    Debug.Trace(count as String)\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_script_properties() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction Test()\n    Debug.Trace(MyValue as String)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn matches_variable_usage_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int total\n    Debug.Trace(TOTAL as String)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Int i\n        Debug.Trace(i as String)\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'i'"));
    }

    #[test]
    fn each_function_starts_fresh() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction First()\n    Int i\n    i = 1\nEndFunction\n\nFunction Second()\n    Int i\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 10);
    }

    #[test]
    fn flags_passing_an_unassigned_variable_as_a_call_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    Debug.Trace(i)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }

    #[test]
    fn does_not_flag_a_gate_comparison_against_the_int_default() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If i == 0\n        i = 5\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_gate_comparison_with_the_default_literal_on_the_left() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If 0 == i\n        i = 5\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_not_equal_gate_comparison_against_the_default() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If i != 0\n        Debug.Trace(\"set\")\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_gate_comparison_against_none() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a\n    If a == None\n        Return\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_gate_comparison_against_the_float_default() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float f\n    If f == 0.0\n        f = 1.0\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_gate_comparison_against_the_bool_default() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Bool b\n    If b == False\n        b = True\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_gate_comparison_against_the_string_default() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    String s\n    If s == \"\"\n        s = \"set\"\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn still_flags_a_comparison_against_a_non_default_value() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If i == 5\n        i = 5\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn still_flags_a_genuine_read_alongside_an_unrelated_gate_comparison() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If i == 0\n        Debug.Trace(i as String)\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }
}
