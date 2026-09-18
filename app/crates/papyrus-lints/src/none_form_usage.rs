//! Flags a member/method access on a local variable that's still known to
//! be `None` at that point (e.g. `Armor a = None` followed by
//! `a.GetName()`), since dereferencing a `None` Form crashes the script at
//! runtime.
//!
//! This works from the parsed AST, tracking which local variables are
//! definitely `None` as it walks each function body in order. A variable
//! becomes known-`None` when declared or assigned a literal `None`, or
//! when it's declared without an initializer at all and its type isn't one
//! of the primitive value types (`Int`/`Float`/`Bool`/`String`) — object-typed
//! locals (`Form` and its subtypes) default to `None` until assigned, unlike
//! primitives which get a non-`None` zero value. Script-level `Auto`/
//! `AutoReadOnly` properties get the same treatment by default: an
//! object-typed one with no explicit initializer (or an explicit `= None`)
//! isn't guaranteed to be filled in until something outside the script (the
//! CK's Property Manager, another script's `PropertyGet`/`PropertySet`,
//! `OnInit`, …) sets it, so each function starts out treating it as
//! possibly `None` too, same as an uninitialized local. Setting
//! [`crate::config::Config::assume_auto_properties_filled`] drops that
//! assumption for properties (not locals), since many projects consider it
//! noise once they trust their Property Manager setup. It stops being
//! tracked as
//! soon as it's assigned anything else, except that assigning it another
//! identifier makes it inherit that identifier's own tracked state instead
//! (so `a = b` keeps `a` known-`None` when `b` still is, rather than
//! clearing it). `If`/`Else`
//! branches are narrowed using the branch's own condition when it's a
//! direct `None` check (`x == None`, `x != None`, `!x`, or a bare `x`,
//! optionally combined with `&&`/`||`), and a branch that unconditionally
//! `Return`s doesn't contribute its exit state to what follows the `If` —
//! covering the common `If x == None \n Return \n EndIf` guard idiom. A
//! `While` loop can only exit when its condition is false (Papyrus has no
//! `break`/`continue`), so the condition is also used to narrow the state
//! after the loop. Anything less direct (a condition built from a call, a
//! member access, or more than one identifier) leaves the state
//! unchanged rather than guessing at it.

use std::collections::HashSet;

use papyrus_parser::ast::{
    AssignOp, BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, TypeName, UnaryOp,
};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "none-form-usage";

/// Checks every function/event in `source` for member/method access on a
/// local variable that's still known to be `None`. When
/// `assume_auto_properties_filled` is `true`, a script-level `Auto`/
/// `AutoReadOnly` property is never treated as possibly `None` on its own
/// (see [`crate::config::Config::assume_auto_properties_filled`]); a local
/// variable's own tracking is unaffected either way.
pub fn check(ast: Option<&Script>, assume_auto_properties_filled: bool) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };

    let default_none_properties = if assume_auto_properties_filled {
        HashSet::new()
    } else {
        default_none_properties(script)
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        let mut none_vars = default_none_properties.clone();
        walk_body(&function.body, &mut none_vars, &mut diagnostics);
    }
    diagnostics
}

/// Script-level `Auto`/`AutoReadOnly` properties that default to `None`
/// until something outside the script (or a later statement) sets them:
/// object-typed ones with no explicit initializer, or an explicit `= None`.
/// Every function starts out treating these the same as an uninitialized
/// local of the same type.
fn default_none_properties(script: &Script) -> HashSet<String> {
    script
        .properties
        .iter()
        .filter(|property| property.is_auto || property.is_auto_read_only)
        .filter(|property| is_object_type(&property.type_name))
        .filter(|property| matches!(&property.value, None | Some(Expr::Literal(Literal::None))))
        .map(|property| property.name.to_lowercase())
        .collect()
}

pub(crate) fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

fn walk_body(body: &[Stmt], none_vars: &mut HashSet<String>, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    check_expr(value, none_vars, diagnostics, decl.line);
                    record_write(&decl.name, value, none_vars);
                } else if is_object_type(&decl.type_name) {
                    none_vars.insert(decl.name.to_lowercase());
                } else {
                    none_vars.remove(&decl.name.to_lowercase());
                }
            }
            Stmt::Assign {
                target,
                op,
                value,
                line,
            } => {
                check_expr(value, none_vars, diagnostics, *line);
                check_expr(target, none_vars, diagnostics, *line);
                if let (Expr::Identifier(name), AssignOp::Assign) = (target, op) {
                    record_write(name, value, none_vars);
                }
            }
            Stmt::Expr { value, line } => check_expr(value, none_vars, diagnostics, *line),
            Stmt::Return {
                value: Some(value),
                line,
            } => check_expr(value, none_vars, diagnostics, *line),
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => handle_if(branches, else_body, none_vars, diagnostics),
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                check_expr(condition, none_vars, diagnostics, *line);
                let mut loop_vars = none_vars.clone();
                narrow_for_truthy(condition, &mut loop_vars);
                walk_body(body, &mut loop_vars, diagnostics);
                // A `While` loop can only exit when its condition is
                // false, since this language has no `break`/`continue`.
                narrow_for_falsy(condition, none_vars);
            }
        }
    }
}

/// Whether `type_name` is an object type (`Form` or one of its subtypes,
/// i.e. any script/native type other than the primitives) rather than one
/// of Papyrus's primitive value types. Object-typed locals default to
/// `None` when declared without an initializer; primitives get a non-`None`
/// zero value (`0`, `0.0`, `False`, `""`) instead.
pub(crate) fn is_object_type(type_name: &TypeName) -> bool {
    !type_name.is_array
        && !matches!(
            type_name.name.to_lowercase().as_str(),
            "int" | "float" | "bool" | "string"
        )
}

/// Updates `none_vars` for a plain `name = value` write (a declaration's
/// initializer or a `Stmt::Assign` with [`AssignOp::Assign`]): known-`None`
/// if `value` is the `None` literal, known-not-`None` otherwise — except
/// when `value` is itself an identifier, in which case `name` inherits
/// that identifier's current tracked state (aliasing a known-`None`
/// variable makes the target known-`None` too, rather than clearing it).
fn record_write(name: &str, value: &Expr, none_vars: &mut HashSet<String>) {
    let key = name.to_lowercase();
    let becomes_none = match value {
        Expr::Literal(Literal::None) => true,
        Expr::Identifier(source) => none_vars.contains(&source.to_lowercase()),
        _ => false,
    };
    if becomes_none {
        none_vars.insert(key);
    } else {
        none_vars.remove(&key);
    }
}

/// Handles an `If`/`ElseIf`/`Else` chain: each branch (and the trailing
/// `Else`, if any) is checked with the incoming state narrowed by that
/// branch's own condition, and only branches that don't unconditionally
/// `Return` contribute their exit state to what follows the `If` — a
/// variable stays known-`None` afterward if it's still `None` along any
/// surviving path.
fn handle_if(
    branches: &[IfBranch],
    else_body: &[Stmt],
    none_vars: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_vars = none_vars.clone();
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

    *none_vars = if surviving.is_empty() {
        // Every branch (including the implicit/explicit else) returns, so
        // nothing after the `If` is reached through it; keep the
        // pre-`If` state rather than guess.
        entry_vars
    } else {
        surviving.into_iter().flatten().collect()
    };
}

/// Whether `body` unconditionally exits its enclosing function, judged
/// (conservatively) by its last statement being a `Return`.
pub(crate) fn diverges(body: &[Stmt]) -> bool {
    matches!(body.last(), Some(Stmt::Return { .. }))
}

/// If `expr` is a direct `None` check on an identifier (`x == None`,
/// `None == x`, `x != None`, `!x`, or a bare `x`), returns its name
/// (lowercased) along with whether `expr` being *true* means that variable
/// is `None`.
fn none_check(expr: &Expr) -> Option<(String, bool)> {
    match expr {
        Expr::Identifier(name) => Some((name.to_lowercase(), false)),
        Expr::Unary {
            op: UnaryOp::Not,
            operand,
        } => none_check(operand).map(|(name, means_none)| (name, !means_none)),
        Expr::Binary {
            left,
            op: BinaryOp::Eq,
            right,
        } => none_literal_compare(left, right, true),
        Expr::Binary {
            left,
            op: BinaryOp::NotEq,
            right,
        } => none_literal_compare(left, right, false),
        _ => None,
    }
}

fn none_literal_compare(
    left: &Expr,
    right: &Expr,
    means_none_if_true: bool,
) -> Option<(String, bool)> {
    match (left, right) {
        (Expr::Identifier(name), Expr::Literal(Literal::None))
        | (Expr::Literal(Literal::None), Expr::Identifier(name)) => {
            Some((name.to_lowercase(), means_none_if_true))
        }
        _ => None,
    }
}

/// Narrows `state` to reflect `condition` having evaluated `true`,
/// recursing into `&&` operands (both must hold).
pub(crate) fn narrow_for_truthy(condition: &Expr, state: &mut HashSet<String>) {
    if let Some((name, means_none)) = none_check(condition) {
        if means_none {
            state.insert(name);
        } else {
            state.remove(&name);
        }
        return;
    }
    if let Expr::Binary {
        left,
        op: BinaryOp::And,
        right,
    } = condition
    {
        narrow_for_truthy(left, state);
        narrow_for_truthy(right, state);
    }
}

/// Narrows `state` to reflect `condition` having evaluated `false`,
/// recursing into `||` operands (both must have been false).
pub(crate) fn narrow_for_falsy(condition: &Expr, state: &mut HashSet<String>) {
    if let Some((name, means_none)) = none_check(condition) {
        if means_none {
            state.remove(&name);
        } else {
            state.insert(name);
        }
        return;
    }
    if let Expr::Binary {
        left,
        op: BinaryOp::Or,
        right,
    } = condition
    {
        narrow_for_falsy(left, state);
        narrow_for_falsy(right, state);
    }
}

/// Recursively checks `expr` for a member/method access on a variable
/// currently in `none_vars`.
fn check_expr(
    expr: &Expr,
    none_vars: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    line: usize,
) {
    match expr {
        Expr::Member { object, property } => {
            check_expr(object, none_vars, diagnostics, line);
            if let Expr::Identifier(name) = &**object {
                if none_vars.contains(&name.to_lowercase()) {
                    diagnostics.push(Diagnostic {
                        line,
                        column: 1,
                        message: format!(
                            "[warning] '{name}' may still be None here; accessing '.{property}' on it will crash the script"
                        ),
                        rule: RULE,
                    });
                }
            }
        }
        Expr::Call { callee, args, .. } => {
            check_expr(callee, none_vars, diagnostics, line);
            for arg in args {
                check_expr(arg, none_vars, diagnostics, line);
            }
        }
        Expr::Binary {
            left,
            op: BinaryOp::And,
            right,
        } => {
            check_expr(left, none_vars, diagnostics, line);
            // Short-circuit: `right` only evaluates once `left` is truthy.
            let mut narrowed = none_vars.clone();
            narrow_for_truthy(left, &mut narrowed);
            check_expr(right, &narrowed, diagnostics, line);
        }
        Expr::Binary {
            left,
            op: BinaryOp::Or,
            right,
        } => {
            check_expr(left, none_vars, diagnostics, line);
            // Short-circuit: `right` only evaluates once `left` is falsy.
            let mut narrowed = none_vars.clone();
            narrow_for_falsy(left, &mut narrowed);
            check_expr(right, &narrowed, diagnostics, line);
        }
        Expr::Binary { left, right, .. } => {
            check_expr(left, none_vars, diagnostics, line);
            check_expr(right, none_vars, diagnostics, line);
        }
        Expr::Unary { operand, .. } => check_expr(operand, none_vars, diagnostics, line),
        Expr::Index { object, index } => {
            check_expr(object, none_vars, diagnostics, line);
            check_expr(index, none_vars, diagnostics, line);
        }
        Expr::Cast { value, .. } => check_expr(value, none_vars, diagnostics, line),
        Expr::NewArray { size, .. } => check_expr(size, none_vars, diagnostics, line),
        Expr::NamedArg { value, .. } => check_expr(value, none_vars, diagnostics, line),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

#[cfg(test)]
#[path = "none_form_usage_tests.rs"]
mod tests;
