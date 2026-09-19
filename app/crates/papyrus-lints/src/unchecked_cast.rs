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
//! directly inline (`(expr as Type).Member`) is flagged unless the same
//! `&&` expression's left-hand side already proved it non-`None` (a bare
//! `expr as Type`, or `expr as Type != None`, evaluated earlier in the
//! same left-to-right, short-circuiting `&&` chain) — since `&&` only
//! evaluates its right side once the left side is truthy, `(akSource as
//! Spell && (akSource as Spell).isHostile())` never dereferences a
//! `None`. Nothing else (a separate statement, an unrelated cast target,
//! or `||`, whose right side runs precisely when the left side is *not*
//! truthy) grants that same guarantee, so those are still flagged.
//!
//! A cast written on a line CreationKit itself generated (see
//! [`crate::fragment_code`]) is never tracked as unchecked in the first
//! place: CreationKit's fragment boilerplate always casts its speaker/
//! actor parameter to a narrower type (e.g. `Actor akSpeaker = akSpeakerRef
//! as Actor`) immediately before the user's own editable code, and
//! guarantees that cast succeeds, so flagging every use of the resulting
//! variable throughout the fragment would just be noise the user can't
//! even silence by adding a `None` check without CreationKit rejecting the
//! edit.

use std::collections::HashSet;

use papyrus_parser::ast::{
    AssignOp, BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, UnaryOp,
};

use crate::fragment_code;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unchecked-cast";

pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::from_ast(lint_issues)
}

/// Checks every function/event in `source` for a member/method access on
/// an unchecked `as` cast result.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

fn lint_issues(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut dyn crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (tokens, config, external);

    let Some(script) = ast else {
        return Vec::new();
    };

    let protected = fragment_code::protected_lines(source);

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        let mut unchecked_vars = HashSet::new();
        walk_body(
            &function.body,
            &protected,
            &mut unchecked_vars,
            &mut diagnostics,
        );
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
    protected: &[bool],
    unchecked_vars: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    check_expr(value, unchecked_vars, &[], diagnostics, decl.line);
                    record_write(&decl.name, value, decl.line, protected, unchecked_vars);
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
                check_expr(value, unchecked_vars, &[], diagnostics, *line);
                check_expr(target, unchecked_vars, &[], diagnostics, *line);
                if let (Expr::Identifier(name), AssignOp::Assign) = (target, op) {
                    record_write(name, value, *line, protected, unchecked_vars);
                }
            }
            Stmt::Expr { value, line } => {
                check_expr(value, unchecked_vars, &[], diagnostics, *line)
            }
            Stmt::Return {
                value: Some(value),
                line,
            } => check_expr(value, unchecked_vars, &[], diagnostics, *line),
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => handle_if(branches, else_body, protected, unchecked_vars, diagnostics),
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                check_expr(condition, unchecked_vars, &[], diagnostics, *line);
                clear_checked(condition, unchecked_vars);
                walk_body(body, protected, unchecked_vars, diagnostics);
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
    protected: &[bool],
    unchecked_vars: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut after_conditions = unchecked_vars.clone();
    let mut surviving = Vec::new();

    for branch in branches {
        check_expr(
            &branch.condition,
            &after_conditions,
            &[],
            diagnostics,
            branch.line,
        );
        clear_checked(&branch.condition, &mut after_conditions);
        let mut branch_vars = after_conditions.clone();
        walk_body(&branch.body, protected, &mut branch_vars, diagnostics);
        if !diverges(&branch.body) {
            surviving.push(branch_vars);
        }
    }

    let mut else_vars = after_conditions.clone();
    walk_body(else_body, protected, &mut else_vars, diagnostics);
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
/// otherwise. A cast written on a `protected` line (CreationKit's own
/// fragment boilerplate; see [`fragment_code::protected_lines`]) is never
/// tracked as unchecked, since CreationKit guarantees that cast succeeds.
fn record_write(
    name: &str,
    value: &Expr,
    line: usize,
    protected: &[bool],
    unchecked_vars: &mut HashSet<String>,
) {
    let key = name.to_lowercase();
    if matches!(value, Expr::Cast { .. }) && !protected.get(line).copied().unwrap_or(false) {
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
/// still tracked in `unchecked_vars`. `guarded_casts` lists inline cast
/// expressions that an enclosing `&&`'s left-hand side already proved
/// non-`None` before `expr` (its right-hand side) runs; a `Cast` matching
/// one of these structurally is not flagged.
fn check_expr(
    expr: &Expr,
    unchecked_vars: &HashSet<String>,
    guarded_casts: &[Expr],
    diagnostics: &mut Vec<Diagnostic>,
    line: usize,
) {
    match expr {
        Expr::Member { object, property } => {
            check_expr(object, unchecked_vars, guarded_casts, diagnostics, line);
            match &**object {
                Expr::Cast { type_name, .. } if !guarded_casts.contains(object) => {
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
            check_expr(callee, unchecked_vars, guarded_casts, diagnostics, line);
            for arg in args {
                check_expr(arg, unchecked_vars, guarded_casts, diagnostics, line);
            }
        }
        Expr::Binary {
            left,
            op: BinaryOp::And,
            right,
        } => {
            check_expr(left, unchecked_vars, guarded_casts, diagnostics, line);
            let mut narrowed = guarded_casts.to_vec();
            collect_guarded_casts(left, &mut narrowed);
            check_expr(right, unchecked_vars, &narrowed, diagnostics, line);
        }
        Expr::Binary { left, right, .. } => {
            check_expr(left, unchecked_vars, guarded_casts, diagnostics, line);
            check_expr(right, unchecked_vars, guarded_casts, diagnostics, line);
        }
        Expr::Unary { operand, .. } => {
            check_expr(operand, unchecked_vars, guarded_casts, diagnostics, line)
        }
        Expr::Index { object, index } => {
            check_expr(object, unchecked_vars, guarded_casts, diagnostics, line);
            check_expr(index, unchecked_vars, guarded_casts, diagnostics, line);
        }
        Expr::Cast { value, .. } => {
            check_expr(value, unchecked_vars, guarded_casts, diagnostics, line)
        }
        Expr::NewArray { size, .. } => {
            check_expr(size, unchecked_vars, guarded_casts, diagnostics, line)
        }
        Expr::NamedArg { value, .. } => {
            check_expr(value, unchecked_vars, guarded_casts, diagnostics, line)
        }
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Collects the inline cast expressions that `expr`, evaluated truthily,
/// proves non-`None` — a bare `value as Type`, `value as Type != None` (or
/// reversed), or a nested `&&` combining either — into `casts`. Used by
/// `check_expr`'s `&&` handling to let a right-hand side skip flagging a
/// cast its left-hand side already checked.
fn collect_guarded_casts(expr: &Expr, casts: &mut Vec<Expr>) {
    match expr {
        Expr::Cast { .. } => casts.push(expr.clone()),
        Expr::Binary {
            left,
            op: BinaryOp::NotEq,
            right,
        } => {
            if matches!(**right, Expr::Literal(Literal::None)) {
                collect_guarded_casts(left, casts);
            } else if matches!(**left, Expr::Literal(Literal::None)) {
                collect_guarded_casts(right, casts);
            }
        }
        Expr::Binary {
            left,
            op: BinaryOp::And,
            right,
        } => {
            collect_guarded_casts(left, casts);
            collect_guarded_casts(right, casts);
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "unchecked_cast_tests.rs"]
mod tests;
