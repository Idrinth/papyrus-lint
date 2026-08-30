//! Flags a member/method access on a `Form`-typed function parameter
//! (e.g. `Function Test(Armor akArmor) \n akArmor.GetName() \n EndFunction`)
//! that hasn't yet been confirmed non-`None` in that path, since a caller
//! can always pass in `None` and dereferencing it crashes the script at
//! runtime.
//!
//! This reuses [`none_form_usage`]'s AST-based dataflow, but tracks the
//! opposite direction: every object-typed (`Form` and its subtypes)
//! parameter starts out "unchecked" instead of a local starting out
//! "known-`None`". A parameter stops being tracked once it's narrowed by a
//! direct `None` check (`x == None`, `x != None`, `!x`, or a bare `x`,
//! optionally combined with `&&`/`||`) the same way `none_form_usage`
//! narrows its own state, or once it's reassigned to anything, since the
//! value being read from then on is no longer the original,
//! possibly-`None` argument. Disabled by default, since many scripts
//! intentionally accept a possibly-`None` Form and defer the check to a
//! caller or a later branch.

use std::collections::HashSet;

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Stmt};

use crate::none_form_usage::{
    all_functions, diverges, is_object_type, narrow_for_falsy, narrow_for_truthy,
};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unchecked-form-parameter";

/// Checks every function/event in `source` for member/method access on a
/// `Form`-typed parameter that hasn't yet been confirmed non-`None`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        let mut unchecked = form_params(function);
        if unchecked.is_empty() {
            continue;
        }
        walk_body(&function.body, &mut unchecked, &mut diagnostics);
    }
    diagnostics
}

/// The (lowercased) names of `function`'s object-typed (`Form` and its
/// subtypes, not arrays or primitives) parameters.
fn form_params(function: &FunctionDecl) -> HashSet<String> {
    function
        .params
        .iter()
        .filter(|param| is_object_type(&param.type_name))
        .map(|param| param.name.to_lowercase())
        .collect()
}

fn walk_body(body: &[Stmt], unchecked: &mut HashSet<String>, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    check_expr(value, unchecked, diagnostics, decl.line);
                }
            }
            Stmt::Assign {
                target,
                value,
                line,
                ..
            } => {
                check_expr(value, unchecked, diagnostics, *line);
                check_expr(target, unchecked, diagnostics, *line);
                if let Expr::Identifier(name) = target {
                    // The parameter has been reassigned; whatever it holds
                    // now is no longer the original, possibly-None
                    // argument, so stop tracking it.
                    unchecked.remove(&name.to_lowercase());
                }
            }
            Stmt::Expr { value, line } => check_expr(value, unchecked, diagnostics, *line),
            Stmt::Return {
                value: Some(value),
                line,
            } => check_expr(value, unchecked, diagnostics, *line),
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => handle_if(branches, else_body, unchecked, diagnostics),
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                check_expr(condition, unchecked, diagnostics, *line);
                let mut loop_vars = unchecked.clone();
                narrow_for_truthy(condition, &mut loop_vars);
                walk_body(body, &mut loop_vars, diagnostics);
                // A `While` loop can only exit when its condition is
                // false, since this language has no `break`/`continue`.
                narrow_for_falsy(condition, unchecked);
            }
        }
    }
}

/// Handles an `If`/`ElseIf`/`Else` chain the same way
/// [`none_form_usage::handle_if`] does: each branch (and the trailing
/// `Else`, if any) is checked with the incoming state narrowed by that
/// branch's own condition, and only branches that don't unconditionally
/// `Return` contribute their exit state to what follows the `If` — a
/// parameter stays unchecked afterward if it's still unchecked along any
/// surviving path.
fn handle_if(
    branches: &[IfBranch],
    else_body: &[Stmt],
    unchecked: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_vars = unchecked.clone();
    let mut surviving = Vec::new();

    for branch in branches {
        check_expr(&branch.condition, &entry_vars, diagnostics, branch.line);
        let mut branch_vars = entry_vars.clone();
        narrow_for_truthy(&branch.condition, &mut branch_vars);
        walk_body(&branch.body, &mut branch_vars, diagnostics);
        if !diverges(&branch.body) {
            surviving.push(branch_vars);
        }
    }

    let mut else_vars = entry_vars.clone();
    if let [only_branch] = branches {
        narrow_for_falsy(&only_branch.condition, &mut else_vars);
    }
    walk_body(else_body, &mut else_vars, diagnostics);
    if !diverges(else_body) {
        surviving.push(else_vars);
    }

    *unchecked = if surviving.is_empty() {
        // Every branch (including the implicit/explicit else) returns, so
        // nothing after the `If` is reached through it; keep the
        // pre-`If` state rather than guess.
        entry_vars
    } else {
        surviving.into_iter().flatten().collect()
    };
}

/// Recursively checks `expr` for a member/method access on a parameter
/// currently in `unchecked`. Passing the parameter on as a plain argument
/// to another call isn't flagged, only a direct member/method access is.
fn check_expr(
    expr: &Expr,
    unchecked: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    line: usize,
) {
    match expr {
        Expr::Member { object, property } => {
            check_expr(object, unchecked, diagnostics, line);
            if let Expr::Identifier(name) = &**object {
                if unchecked.contains(&name.to_lowercase()) {
                    diagnostics.push(Diagnostic {
                        line,
                        column: 1,
                        message: format!(
                            "[warning] parameter '{name}' may be None; accessing '.{property}' on it without a None check will crash the script"
                        ),
                        rule: RULE,
                    });
                }
            }
        }
        Expr::Call { callee, args, .. } => {
            check_expr(callee, unchecked, diagnostics, line);
            for arg in args {
                check_expr(arg, unchecked, diagnostics, line);
            }
        }
        Expr::Binary {
            left,
            op: BinaryOp::And,
            right,
        } => {
            check_expr(left, unchecked, diagnostics, line);
            // Short-circuit: `right` only evaluates once `left` is truthy.
            let mut narrowed = unchecked.clone();
            narrow_for_truthy(left, &mut narrowed);
            check_expr(right, &narrowed, diagnostics, line);
        }
        Expr::Binary {
            left,
            op: BinaryOp::Or,
            right,
        } => {
            check_expr(left, unchecked, diagnostics, line);
            // Short-circuit: `right` only evaluates once `left` is falsy.
            let mut narrowed = unchecked.clone();
            narrow_for_falsy(left, &mut narrowed);
            check_expr(right, &narrowed, diagnostics, line);
        }
        Expr::Binary { left, right, .. } => {
            check_expr(left, unchecked, diagnostics, line);
            check_expr(right, unchecked, diagnostics, line);
        }
        Expr::Unary { operand, .. } => check_expr(operand, unchecked, diagnostics, line),
        Expr::Index { object, index } => {
            check_expr(object, unchecked, diagnostics, line);
            check_expr(index, unchecked, diagnostics, line);
        }
        Expr::Cast { value, .. } => check_expr(value, unchecked, diagnostics, line),
        Expr::NewArray { size, .. } => check_expr(size, unchecked, diagnostics, line),
        Expr::NamedArg { value, .. } => check_expr(value, unchecked, diagnostics, line),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_method_call_on_unchecked_form_parameter() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    akArmor.GetName()\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("'akArmor'"));
        assert!(diagnostics[0].message.contains(".GetName"));
    }

    #[test]
    fn flags_property_access_on_unchecked_form_parameter() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Debug.Trace(akArmor.Name)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn does_not_flag_primitive_or_array_parameters() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int i, Bool b, String s, Armor[] arr)\n    Debug.Trace(s)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_after_early_return_none_guard() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If akArmor == None\n        Return\n    EndIf\n    akArmor.GetName()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_after_bang_guard() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If !akArmor\n        Return\n    EndIf\n    akArmor.GetName()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_inside_not_equal_none_branch() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If akArmor != None\n        akArmor.GetName()\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_inside_equal_none_branch() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If akArmor == None\n        akArmor.GetName()\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_flag_reassigned_parameter() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    akArmor = Game.GetPlayer() as Armor\n    akArmor.GetName()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_passing_an_unchecked_parameter_as_an_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Debug.Trace(akArmor)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_use_still_possibly_unchecked_after_one_sided_guard() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor, Bool flag)\n    If flag\n        If akArmor == None\n            Return\n        EndIf\n    EndIf\n    akArmor.GetName()\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 9);
    }

    #[test]
    fn does_not_flag_after_while_loop_guard() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    While akArmor == None\n        akArmor = Game.GetPlayer() as Armor\n    EndWhile\n    akArmor.GetName()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Armor akArmor)\n        akArmor.GetName()\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }

    #[test]
    fn does_not_flag_short_circuited_and_guard() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If akArmor && akArmor.GetName() == \"\"\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }
}
