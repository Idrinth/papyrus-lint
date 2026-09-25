//! Flags a member/method access on an array element (e.g. `Actor[] act = new
//! Actor[3]` followed directly by `act[2].Kill()`) that hasn't yet been
//! confirmed non-`None` in that path, since nothing about declaring or
//! sizing an array of `Form`/script-typed elements guarantees any of its
//! positions actually hold something — an unset element reads back as
//! `None`, and dereferencing it crashes the script at runtime.
//!
//! This reuses the same AST-based dataflow shape as
//! [`crate::unchecked_form_parameter`], but keyed by "which array element"
//! instead of "which parameter": every local array variable or array-typed
//! parameter whose element type is an object type (`Form` and its
//! subtypes, not `Int`/`Float`/`Bool`/`String`) is tracked, and a specific
//! element (`array_name` plus a constant-folded index, e.g. `act[2]` or
//! `act[1 + 1]` — the same folding [`crate::array_bounds`] applies to its
//! own indices) starts out unconfirmed the moment it comes into scope. It's
//! narrowed to confirmed-non-`None` by an `If`/`ElseIf`/`Else` branch (or a
//! `While` loop's condition) guarded by a direct `None` check on that exact
//! element (`act[2] == None`, `act[2] != None`, `!act[2]`, or a bare
//! `act[2]`, optionally combined with `&&`/`||`), the same way
//! `unchecked_form_parameter` narrows its own state — including a branch
//! that unconditionally `Return`s not carrying its state past the `If`,
//! covering the common `If act[2] == None` / `Return` guard idiom — and
//! reverts to unconfirmed again the moment that element is assigned a new
//! value, since the value just written could itself be `None`. Only a
//! plain identifier's own element, indexed by a literal (optionally
//! combined with arithmetic, comparison, logical, and unary operators), is
//! tracked; a
//! member/property array, or an index built from anything else (a variable,
//! a call, ...), is left unflagged rather than guessed at, the same
//! restriction [`crate::array_bounds`] places on its own indices. Passing
//! the element on as a plain argument to another call isn't flagged, only a
//! direct member/method access is.

use std::collections::HashSet;

use papyrus_parser::ast::{
    BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Stmt, TypeName, UnaryOp,
};

use crate::const_eval::eval_const_int;
use crate::none_form_usage::diverges;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unchecked-array-element";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        let mut object_arrays = param_object_arrays(function);
        let mut checked = HashSet::new();
        let mut diagnostics = Vec::new();
        walk_body(
            &function.body,
            &mut object_arrays,
            &mut checked,
            &mut diagnostics,
        );
        self.store.extend(diagnostics);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every function/event in `source` for member/method access on an
/// array element that hasn't yet been confirmed non-`None`.
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

/// Whether `type_name` names an array of an object type (`Form` and its
/// subtypes) rather than one of Papyrus's primitive value types. A
/// primitive-element array (`Int[]`, `Float[]`, `Bool[]`, `String[]`) never
/// has a `None` element to begin with, so it's never worth tracking here.
fn is_object_array(type_name: &TypeName) -> bool {
    type_name.is_array
        && !matches!(
            type_name.name.to_lowercase().as_str(),
            "int" | "float" | "bool" | "string"
        )
}

/// The (lowercased) names of `function`'s object-array-typed parameters.
fn param_object_arrays(function: &FunctionDecl) -> HashSet<String> {
    function
        .params
        .iter()
        .filter(|param| is_object_array(&param.type_name))
        .map(|param| param.name.to_lowercase())
        .collect()
}

/// If `expr` is a plain identifier indexed by a constant-foldable integer,
/// returns that identifier's original-case name alongside the folded index
/// value. Anything else (a member/property array, a non-literal index, a
/// nested index, ...) returns `None` rather than guessing at an identity for
/// it.
fn index_ref(expr: &Expr) -> Option<(&str, i64)> {
    let Expr::Index { object, index } = expr else {
        return None;
    };
    let Expr::Identifier(name) = object.as_ref() else {
        return None;
    };
    let value = eval_const_int(index)?;
    Some((name.as_str(), value))
}

/// The canonical, case-insensitive key identifying "this specific array
/// element" across separate reads of it.
fn key_of(name: &str, value: i64) -> String {
    format!("{}[{value}]", name.to_lowercase())
}

fn walk_body(
    body: &[Stmt],
    object_arrays: &mut HashSet<String>,
    checked: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if is_object_array(&decl.type_name) {
                    object_arrays.insert(decl.name.to_lowercase());
                }
                if let Some(value) = &decl.value {
                    check_expr(value, object_arrays, checked, diagnostics, decl.line);
                }
            }
            Stmt::Assign {
                target,
                value,
                line,
                ..
            } => {
                check_expr(value, object_arrays, checked, diagnostics, *line);
                check_expr(target, object_arrays, checked, diagnostics, *line);
                if let Some((name, index)) = index_ref(target) {
                    // The element just got a new value, which could itself
                    // be None, so whatever was confirmed about it before no
                    // longer applies.
                    checked.remove(&key_of(name, index));
                }
            }
            Stmt::Expr { value, line } => {
                check_expr(value, object_arrays, checked, diagnostics, *line)
            }
            Stmt::Return {
                value: Some(value),
                line,
            } => check_expr(value, object_arrays, checked, diagnostics, *line),
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => handle_if(branches, else_body, object_arrays, checked, diagnostics),
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                check_expr(condition, object_arrays, checked, diagnostics, *line);
                let mut loop_checked = checked.clone();
                narrow_for_truthy(condition, &mut loop_checked);
                walk_body(body, object_arrays, &mut loop_checked, diagnostics);
                // A `While` loop can only exit when its condition is
                // false, since this language has no `break`/`continue`.
                narrow_for_falsy(condition, checked);
            }
            Stmt::LockGuard { body, else_body, .. } => {
                let mut locked = checked.clone();
                walk_body(body, object_arrays, &mut locked, diagnostics);
                let mut alternate = checked.clone();
                walk_body(else_body, object_arrays, &mut alternate, diagnostics);
            }
        }
    }
}

/// Handles an `If`/`ElseIf`/`Else` chain: each branch (and the trailing
/// `Else`, if any) is checked with the incoming state narrowed by that
/// branch's own condition, and only branches that don't unconditionally
/// `Return` contribute their exit state to what follows the `If`. Unlike
/// [`crate::unchecked_form_parameter::handle_if`], a confirmed element stays
/// confirmed afterward only when every surviving branch agrees it's
/// confirmed (an intersection, the same merge
/// [`crate::array_bounds::handle_if`] uses for its own tracked sizes) —
/// since here the goal is "has this been proven non-`None` on every path
/// that reaches here", not "is it still possibly `None` on some path".
fn handle_if(
    branches: &[IfBranch],
    else_body: &[Stmt],
    object_arrays: &mut HashSet<String>,
    checked: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_checked = checked.clone();
    let mut surviving = Vec::new();

    for branch in branches {
        check_expr(
            &branch.condition,
            object_arrays,
            &entry_checked,
            diagnostics,
            branch.line,
        );
        let mut branch_checked = entry_checked.clone();
        narrow_for_truthy(&branch.condition, &mut branch_checked);
        walk_body(
            &branch.body,
            object_arrays,
            &mut branch_checked,
            diagnostics,
        );
        if !diverges(&branch.body) {
            surviving.push(branch_checked);
        }
    }

    let mut else_checked = entry_checked.clone();
    if let [only_branch] = branches {
        narrow_for_falsy(&only_branch.condition, &mut else_checked);
    }
    walk_body(else_body, object_arrays, &mut else_checked, diagnostics);
    if !diverges(else_body) {
        surviving.push(else_checked);
    }

    *checked = if surviving.is_empty() {
        // Every branch (including the implicit/explicit else) returns, so
        // nothing after the `If` is reached through it; keep the pre-`If`
        // state rather than guess.
        entry_checked
    } else {
        let mut merged = surviving.swap_remove(0);
        for branch_checked in &surviving {
            merged.retain(|key| branch_checked.contains(key));
        }
        merged
    };
}

/// If `expr` is a direct `None` check on a trackable array element
/// (`arr[N] == None`, `None == arr[N]`, `arr[N] != None`, `!arr[N]`, or a
/// bare `arr[N]`), returns that element's key along with whether `expr`
/// being *true* means that element is `None`.
fn none_check(expr: &Expr) -> Option<(String, bool)> {
    match expr {
        Expr::Index { .. } => {
            let (name, value) = index_ref(expr)?;
            Some((key_of(name, value), false))
        }
        Expr::Unary {
            op: UnaryOp::Not,
            operand,
        } => none_check(operand).map(|(key, means_none)| (key, !means_none)),
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
        (index_expr @ Expr::Index { .. }, Expr::Literal(Literal::None))
        | (Expr::Literal(Literal::None), index_expr @ Expr::Index { .. }) => {
            let (name, value) = index_ref(index_expr)?;
            Some((key_of(name, value), means_none_if_true))
        }
        _ => None,
    }
}

/// Narrows `checked` to reflect `condition` having evaluated `true`,
/// recursing into `&&` operands (both must hold).
fn narrow_for_truthy(condition: &Expr, checked: &mut HashSet<String>) {
    if let Some((key, means_none)) = none_check(condition) {
        if means_none {
            checked.remove(&key);
        } else {
            checked.insert(key);
        }
        return;
    }
    if let Expr::Binary {
        left,
        op: BinaryOp::And,
        right,
    } = condition
    {
        narrow_for_truthy(left, checked);
        narrow_for_truthy(right, checked);
    }
}

/// Narrows `checked` to reflect `condition` having evaluated `false`,
/// recursing into `||` operands (both must have been false).
fn narrow_for_falsy(condition: &Expr, checked: &mut HashSet<String>) {
    if let Some((key, means_none)) = none_check(condition) {
        if means_none {
            checked.insert(key);
        } else {
            checked.remove(&key);
        }
        return;
    }
    if let Expr::Binary {
        left,
        op: BinaryOp::Or,
        right,
    } = condition
    {
        narrow_for_falsy(left, checked);
        narrow_for_falsy(right, checked);
    }
}

/// Recursively checks `expr` for a member/method access on an array element
/// currently unconfirmed in `checked`.
fn check_expr(
    expr: &Expr,
    object_arrays: &HashSet<String>,
    checked: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    line: usize,
) {
    match expr {
        Expr::Member { object, property } => {
            check_expr(object, object_arrays, checked, diagnostics, line);
            if let Some((name, value)) = index_ref(object) {
                if object_arrays.contains(&name.to_lowercase()) {
                    let key = key_of(name, value);
                    if !checked.contains(&key) {
                        diagnostics.push(Diagnostic {
                            line,
                            column: 1,
                            message: format!(
                                "[warning] Element {value} of array '{name}' may be None; \
                                 accessing '.{property}' on it without a None check will crash \
                                 the script"
                            ),
                            rule: RULE,
                        });
                    }
                }
            }
        }
        Expr::Call { callee, args, .. } => {
            check_expr(callee, object_arrays, checked, diagnostics, line);
            for arg in args {
                check_expr(arg, object_arrays, checked, diagnostics, line);
            }
        }
        Expr::Binary {
            left,
            op: BinaryOp::And,
            right,
        } => {
            check_expr(left, object_arrays, checked, diagnostics, line);
            // Short-circuit: `right` only evaluates once `left` is truthy.
            let mut narrowed = checked.clone();
            narrow_for_truthy(left, &mut narrowed);
            check_expr(right, object_arrays, &narrowed, diagnostics, line);
        }
        Expr::Binary {
            left,
            op: BinaryOp::Or,
            right,
        } => {
            check_expr(left, object_arrays, checked, diagnostics, line);
            // Short-circuit: `right` only evaluates once `left` is falsy.
            let mut narrowed = checked.clone();
            narrow_for_falsy(left, &mut narrowed);
            check_expr(right, object_arrays, &narrowed, diagnostics, line);
        }
        Expr::Binary { left, right, .. } => {
            check_expr(left, object_arrays, checked, diagnostics, line);
            check_expr(right, object_arrays, checked, diagnostics, line);
        }
        Expr::Unary { operand, .. } => {
            check_expr(operand, object_arrays, checked, diagnostics, line)
        }
        Expr::Index { object, index } => {
            check_expr(object, object_arrays, checked, diagnostics, line);
            check_expr(index, object_arrays, checked, diagnostics, line);
        }
        Expr::Cast { value, .. } | Expr::Is { value, .. } => check_expr(value, object_arrays, checked, diagnostics, line),
        Expr::NewArray { size, .. } => check_expr(size, object_arrays, checked, diagnostics, line),
        Expr::NamedArg { value, .. } => {
            check_expr(value, object_arrays, checked, diagnostics, line)
        }
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
        Expr::NewStruct { .. } => {}
    }
}

#[cfg(test)]
#[path = "unchecked_array_element_tests.rs"]
mod tests;
