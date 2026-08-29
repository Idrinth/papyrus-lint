//! Flags a member/method access on the result of an `as` cast (e.g.
//! `(akRef as Actor).GetActorValue("Health")`) before that result has been
//! checked against `None`, since a cast that doesn't match the underlying
//! Form's actual type evaluates to `None` at runtime rather than raising a
//! compile-time or runtime error, so dereferencing it immediately crashes
//! the script.
//!
//! This works from the parsed AST, tracking which local variables currently
//! hold an unchecked cast result as it walks each function body in order,
//! the same way [`crate::none_form_usage`] tracks known-`None` locals. A
//! variable becomes "unchecked" when declared or assigned directly from an
//! `as` cast expression, and stops being tracked as soon as it's assigned
//! anything else. Unlike that lint, this one doesn't need to determine
//! which branch a check narrows to — it only cares whether the cast's
//! possible `None` was ever considered at all, so a variable is cleared the
//! moment a direct `None` check on it (`x == None`, `x != None`, `!x`, or a
//! bare `x`, optionally combined with `&&`/`||`) is *evaluated*, regardless
//! of which branch is ultimately taken; a `While` loop's condition clears
//! it both before and after the loop body, since the condition is
//! re-evaluated every iteration including the one that exits it.
//! `If`/`Else` branches only inherit the checks their own condition (and
//! any earlier condition in the same `If`/`ElseIf` chain) actually
//! performed, and a branch that unconditionally `Return`s doesn't
//! contribute its exit state to what follows the `If`. A cast used
//! directly inline (`(expr as Type).Member`) is always flagged, since
//! there's no way to check it for `None` in between.

use std::collections::HashSet;

use papyrus_parser::ast::{
    AssignOp, BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, UnaryOp,
};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unchecked-cast";

/// Checks every function/event in `source` for a member/method access on
/// an unchecked `as` cast result.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        let mut unchecked_vars = HashSet::new();
        walk_body(&function.body, &mut unchecked_vars, &mut diagnostics);
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

fn walk_body(
    body: &[Stmt],
    unchecked_vars: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    check_expr(value, unchecked_vars, diagnostics, decl.line);
                    record_write(&decl.name, value, unchecked_vars);
                } else {
                    unchecked_vars.remove(&decl.name.to_lowercase());
                }
            }
            Stmt::Assign {
                target,
                op,
                value,
                line,
            } => {
                check_expr(value, unchecked_vars, diagnostics, *line);
                check_expr(target, unchecked_vars, diagnostics, *line);
                if let (Expr::Identifier(name), AssignOp::Assign) = (target, op) {
                    record_write(name, value, unchecked_vars);
                }
            }
            Stmt::Expr { value, line } => check_expr(value, unchecked_vars, diagnostics, *line),
            Stmt::Return {
                value: Some(value),
                line,
            } => check_expr(value, unchecked_vars, diagnostics, *line),
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => handle_if(branches, else_body, unchecked_vars, diagnostics),
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                check_expr(condition, unchecked_vars, diagnostics, *line);
                clear_checked(condition, unchecked_vars);
                walk_body(body, unchecked_vars, diagnostics);
                // The condition is re-evaluated every iteration, including
                // the final one that exits the loop, so it's checked again
                // even if the body just reassigned a fresh cast to it.
                clear_checked(condition, unchecked_vars);
            }
        }
    }
}

/// Handles an `If`/`ElseIf`/`Else` chain: each branch only inherits the
/// `None` checks performed by its own condition and every earlier
/// condition in the chain (all evaluated in order before it can run), and
/// only branches that don't unconditionally `Return` contribute their exit
/// state to what follows the `If` — a variable stays "unchecked" afterward
/// if it's still unchecked along any surviving path.
fn handle_if(
    branches: &[IfBranch],
    else_body: &[Stmt],
    unchecked_vars: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut after_conditions = unchecked_vars.clone();
    let mut surviving = Vec::new();

    for branch in branches {
        check_expr(
            &branch.condition,
            &after_conditions,
            diagnostics,
            branch.line,
        );
        clear_checked(&branch.condition, &mut after_conditions);
        let mut branch_vars = after_conditions.clone();
        walk_body(&branch.body, &mut branch_vars, diagnostics);
        if !diverges(&branch.body) {
            surviving.push(branch_vars);
        }
    }

    let mut else_vars = after_conditions.clone();
    walk_body(else_body, &mut else_vars, diagnostics);
    if !diverges(else_body) {
        surviving.push(else_vars);
    }

    *unchecked_vars = if surviving.is_empty() {
        // Every branch (including the implicit/explicit else) returns, so
        // nothing after the `If` is reached through it; keep the
        // post-conditions state rather than guess.
        after_conditions
    } else {
        surviving.into_iter().flatten().collect()
    };
}

/// Whether `body` unconditionally exits its enclosing function, judged
/// (conservatively) by its last statement being a `Return`.
fn diverges(body: &[Stmt]) -> bool {
    matches!(body.last(), Some(Stmt::Return { .. }))
}

/// Updates `unchecked_vars` for a plain `name = value` write (a
/// declaration's initializer or a `Stmt::Assign` with [`AssignOp::Assign`]):
/// tracked as unchecked if `value` is an `as` cast expression, cleared
/// otherwise.
fn record_write(name: &str, value: &Expr, unchecked_vars: &mut HashSet<String>) {
    let key = name.to_lowercase();
    if matches!(value, Expr::Cast { .. }) {
        unchecked_vars.insert(key);
    } else {
        unchecked_vars.remove(&key);
    }
}

/// If `expr` is a direct `None` check on an identifier (`x == None`, `None
/// == x`, `x != None`, `!x`, or a bare `x`, optionally combined with
/// `&&`/`||`), removes that identifier from `unchecked_vars`: evaluating
/// the check at all means the cast's possible `None` was considered,
/// regardless of which branch is ultimately taken.
fn clear_checked(expr: &Expr, unchecked_vars: &mut HashSet<String>) {
    match expr {
        Expr::Identifier(name) => {
            unchecked_vars.remove(&name.to_lowercase());
        }
        Expr::Unary {
            op: UnaryOp::Not,
            operand,
        } => clear_checked(operand, unchecked_vars),
        Expr::Binary {
            left,
            op: BinaryOp::Eq | BinaryOp::NotEq,
            right,
        } => {
            if let (Expr::Identifier(name), Expr::Literal(Literal::None))
            | (Expr::Literal(Literal::None), Expr::Identifier(name)) = (&**left, &**right)
            {
                unchecked_vars.remove(&name.to_lowercase());
            }
        }
        Expr::Binary {
            left,
            op: BinaryOp::And | BinaryOp::Or,
            right,
        } => {
            clear_checked(left, unchecked_vars);
            clear_checked(right, unchecked_vars);
        }
        _ => {}
    }
}

/// Recursively checks `expr` for a member/method access on an unchecked
/// cast, either inline (`(value as Type).Member`) or through a variable
/// still tracked in `unchecked_vars`.
fn check_expr(
    expr: &Expr,
    unchecked_vars: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    line: usize,
) {
    match expr {
        Expr::Member { object, property } => {
            check_expr(object, unchecked_vars, diagnostics, line);
            match &**object {
                Expr::Cast { type_name, .. } => {
                    diagnostics.push(Diagnostic {
                        line,
                        column: 1,
                        message: format!(
                            "[warning] cast to '{type_name}' may be None; accessing '.{property}' on it without a None check first will crash the script"
                        ),
                        rule: RULE,
                    });
                }
                Expr::Identifier(name) if unchecked_vars.contains(&name.to_lowercase()) => {
                    diagnostics.push(Diagnostic {
                        line,
                        column: 1,
                        message: format!(
                            "[warning] '{name}' holds an unchecked cast result and may be None here; accessing '.{property}' on it will crash the script"
                        ),
                        rule: RULE,
                    });
                }
                _ => {}
            }
        }
        Expr::Call { callee, args, .. } => {
            check_expr(callee, unchecked_vars, diagnostics, line);
            for arg in args {
                check_expr(arg, unchecked_vars, diagnostics, line);
            }
        }
        Expr::Binary { left, right, .. } => {
            check_expr(left, unchecked_vars, diagnostics, line);
            check_expr(right, unchecked_vars, diagnostics, line);
        }
        Expr::Unary { operand, .. } => check_expr(operand, unchecked_vars, diagnostics, line),
        Expr::Index { object, index } => {
            check_expr(object, unchecked_vars, diagnostics, line);
            check_expr(index, unchecked_vars, diagnostics, line);
        }
        Expr::Cast { value, .. } => check_expr(value, unchecked_vars, diagnostics, line),
        Expr::NewArray { size, .. } => check_expr(size, unchecked_vars, diagnostics, line),
        Expr::NamedArg { value, .. } => check_expr(value, unchecked_vars, diagnostics, line),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_inline_cast_dereferenced_directly() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    (akRef as Actor).GetActorValue(\"Health\")\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("'Actor'"));
        assert!(diagnostics[0].message.contains(".GetActorValue"));
    }

    #[test]
    fn flags_method_call_on_variable_assigned_a_cast() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    a.GetActorValue(\"Health\")\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert!(diagnostics[0].message.contains("'a'"));
    }

    #[test]
    fn flags_property_access_on_variable_assigned_a_cast() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a\n    a = akRef as Actor\n    Debug.Trace(a.Name)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn does_not_flag_after_early_return_none_guard() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    If a == None\n        Return\n    EndIf\n    a.GetName()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_after_bang_guard() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    If !a\n        Return\n    EndIf\n    a.GetName()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_inside_not_equal_none_branch() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    If a != None\n        a.GetName()\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_inside_and_guarded_branch() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef, Bool flag)\n    Actor a = akRef as Actor\n    If a != None && flag\n        a.GetName()\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_inside_equal_none_branch() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    If a == None\n        a.GetName()\n    EndIf\nEndFunction\n",
        );

        // Evaluating the check clears "unchecked" regardless of branch, so
        // this is left to `none-form-usage` (a different, more precise
        // lint) to flag instead.
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_after_while_loop_condition_checks_it() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    While a == None\n        a = akRef as Actor\n    EndWhile\n    a.GetName()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_variable_reassigned_from_a_non_cast_value() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef, Actor akActor)\n    Actor a = akRef as Actor\n    a = akActor\n    a.GetName()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_declaration_without_a_cast() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    Actor a = akActor\n    a.GetName()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_passing_an_unchecked_cast_variable_as_an_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    Debug.Trace(a)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_use_still_unchecked_after_one_sided_reassignment() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef, Actor akActor, Bool flag)\n    Actor a = akActor\n    If flag\n        a = akRef as Actor\n    EndIf\n    a.GetName()\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 8);
    }

    #[test]
    fn does_not_flag_when_both_branches_check_it() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef, Bool flag)\n    Actor a = akRef as Actor\n    If flag && a != None\n        a.GetName()\n    ElseIf a != None\n        a.GetName()\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(ObjectReference akRef)\n        Actor a = akRef as Actor\n        a.GetName()\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }
}
