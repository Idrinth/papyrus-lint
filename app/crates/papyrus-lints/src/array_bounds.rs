//! Flags a literal index into a local array variable that falls outside the
//! compile-time-constant size it was declared with (e.g. `Float[] a = new
//! Float[3]` followed by `a[5] = 0.1`), since Papyrus doesn't raise a
//! catchable error for an out-of-range array access — it just logs the
//! mistake and silently no-ops the write or returns the type's default for
//! a read.
//!
//! This works from the parsed AST, tracking which local variables currently
//! hold an array of a known constant size as it walks each function body in
//! order, the same flow-sensitive shape [`crate::variable_used_before_assignment`]
//! uses for tracking still-unassigned locals: a variable starts tracking a
//! size the moment it's assigned `new <Type>[<N>]` for a literal (or
//! literal-arithmetic) `N`, stops being tracked the moment it's assigned
//! anything else (a different array whose size isn't foldable, or a
//! non-array value entirely), and — like that lint's `If`/`ElseIf`/`Else`
//! handling — only keeps a size past an `If` when every surviving branch
//! agrees on it. A `While` loop may run zero times, so a size learned only
//! inside its body is never assumed to still hold once execution reaches
//! the code after the loop. Only a plain identifier's own index is checked;
//! a member/property array, or an index built from anything other than a
//! literal (optionally combined with arithmetic and unary operators), is
//! left unflagged rather than guessed at.
//!
//! A `new <Type>[<N>]` whose own literal `N` falls outside the range
//! Papyrus allows for a script-created array is flagged separately, by
//! [`crate::array_size_range`], regardless of whether the resulting
//! array's size ends up tracked here.

use std::collections::HashMap;

use papyrus_parser::ast::{AssignOp, BinaryOp, Expr, IfBranch, Literal, Script, Stmt, UnaryOp};

use crate::none_form_usage::{all_functions, diverges};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "array-bounds";

/// Checks every function/event in `source` for a literal array index that
/// falls outside the constant size the array was declared with.
pub fn check(ast: Option<&Script>) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        let mut sizes = HashMap::new();
        walk_body(&function.body, &mut sizes, &mut diagnostics);
    }
    diagnostics
}

fn walk_body(body: &[Stmt], sizes: &mut HashMap<String, i64>, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    check_expr(value, sizes, diagnostics, decl.line);
                    track_assignment(&decl.name, value, sizes);
                } else {
                    sizes.remove(&decl.name.to_lowercase());
                }
            }
            Stmt::Assign {
                target,
                op,
                value,
                line,
            } => {
                check_expr(value, sizes, diagnostics, *line);
                match (target, op) {
                    (Expr::Identifier(name), AssignOp::Assign) => {
                        track_assignment(name, value, sizes);
                    }
                    (Expr::Identifier(name), _) => {
                        // A compound assignment (`x += 1`, ...) never applies
                        // to an array itself, so the target is no longer one
                        // whose size is known this way.
                        sizes.remove(&name.to_lowercase());
                    }
                    _ => check_expr(target, sizes, diagnostics, *line),
                }
            }
            Stmt::Expr { value, line } => check_expr(value, sizes, diagnostics, *line),
            Stmt::Return {
                value: Some(value),
                line,
            } => check_expr(value, sizes, diagnostics, *line),
            Stmt::Return { value: None, .. } => {}
            Stmt::If {
                branches,
                else_body,
                ..
            } => handle_if(branches, else_body, sizes, diagnostics),
            Stmt::While {
                condition,
                body,
                line,
                ..
            } => {
                check_expr(condition, sizes, diagnostics, *line);
                let mut loop_sizes = sizes.clone();
                walk_body(body, &mut loop_sizes, diagnostics);
                // A `While` loop may run zero times, so a size learned only
                // inside its body can't be assumed to still hold once
                // execution reaches the code after the loop.
            }
        }
    }
}

/// Handles an `If`/`ElseIf`/`Else` chain the same way
/// [`crate::variable_used_before_assignment::handle_if`] does: each branch
/// (and the trailing `Else`, if any) is checked from the same incoming
/// state, and only branches that don't unconditionally `Return` contribute
/// their exit state to what follows the `If`. A variable's size is kept
/// afterward only when every surviving branch agrees on the very same size;
/// a branch leaving it untracked (or set to a different size) drops it.
fn handle_if(
    branches: &[IfBranch],
    else_body: &[Stmt],
    sizes: &mut HashMap<String, i64>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_sizes = sizes.clone();
    let mut surviving = Vec::new();

    for branch in branches {
        check_expr(&branch.condition, &entry_sizes, diagnostics, branch.line);
        let mut branch_sizes = entry_sizes.clone();
        walk_body(&branch.body, &mut branch_sizes, diagnostics);
        if !diverges(&branch.body) {
            surviving.push(branch_sizes);
        }
    }

    let mut else_sizes = entry_sizes.clone();
    walk_body(else_body, &mut else_sizes, diagnostics);
    if !diverges(else_body) {
        surviving.push(else_sizes);
    }

    *sizes = if surviving.is_empty() {
        entry_sizes
    } else {
        let mut merged = surviving.swap_remove(0);
        for branch_sizes in &surviving {
            merged.retain(|name, size| branch_sizes.get(name) == Some(size));
        }
        merged
    };
}

/// Updates `sizes` for an assignment of `value` to `name`: records the
/// constant size when `value` is `new <Type>[<N>]` for a non-negative,
/// literal-foldable `N`, and otherwise stops tracking `name` (it either
/// isn't an array anymore, or its size can't be determined at lint time).
fn track_assignment(name: &str, value: &Expr, sizes: &mut HashMap<String, i64>) {
    match array_literal_size(value) {
        Some(size) => {
            sizes.insert(name.to_lowercase(), size);
        }
        None => {
            sizes.remove(&name.to_lowercase());
        }
    }
}

fn array_literal_size(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::NewArray { size, .. } => match eval_const_int(size) {
            Some(size) if size >= 0 => Some(size),
            _ => None,
        },
        _ => None,
    }
}

/// Recursively checks `expr` for an index into a tracked-size array
/// identifier that falls outside `0..size`, and recurses into every
/// sub-expression (so a nested index, e.g. `a[b[0]]`, checks both).
fn check_expr(
    expr: &Expr,
    sizes: &HashMap<String, i64>,
    diagnostics: &mut Vec<Diagnostic>,
    line: usize,
) {
    match expr {
        Expr::Index { object, index } => {
            check_expr(object, sizes, diagnostics, line);
            check_expr(index, sizes, diagnostics, line);
            if let Expr::Identifier(name) = object.as_ref() {
                if let Some(&size) = sizes.get(&name.to_lowercase()) {
                    if let Some(literal_index) = eval_const_int(index) {
                        if literal_index < 0 || literal_index >= size {
                            diagnostics.push(Diagnostic {
                                line,
                                column: 1,
                                message: format!(
                                    "[warning] Index {literal_index} is out of bounds for \
                                     array '{name}', which was declared with size {size}"
                                ),
                                rule: RULE,
                            });
                        }
                    }
                }
            }
        }
        Expr::Member { object, .. } => check_expr(object, sizes, diagnostics, line),
        Expr::Call { callee, args, .. } => {
            check_expr(callee, sizes, diagnostics, line);
            for arg in args {
                check_expr(arg, sizes, diagnostics, line);
            }
        }
        Expr::Binary { left, right, .. } => {
            check_expr(left, sizes, diagnostics, line);
            check_expr(right, sizes, diagnostics, line);
        }
        Expr::Unary { operand, .. } => check_expr(operand, sizes, diagnostics, line),
        Expr::Cast { value, .. } => check_expr(value, sizes, diagnostics, line),
        Expr::NewArray { size, .. } => {
            check_expr(size, sizes, diagnostics, line);
        }
        Expr::NamedArg { value, .. } => check_expr(value, sizes, diagnostics, line),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Attempts to fold `expr` down to a single constant `Int`, returning `None`
/// as soon as any part of it depends on something that can't be known
/// without running the script (an identifier, a call, `Self`/`Parent`, a
/// member/index access, a cast, a `new` array, a `Float`, division, or
/// modulo).
///
/// `pub(crate)` rather than private so [`crate::unchecked_array_element`] can
/// fold an index expression to the same constant this lint would, keeping
/// the two lints' notion of "the same array element" (e.g. `a[2]` and
/// `a[1 + 1]`) identical.
pub(crate) fn eval_const_int(expr: &Expr) -> Option<i64> {
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
#[path = "array_bounds_tests.rs"]
mod tests;
