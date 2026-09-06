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
//! Independent of that tracked-size check, a `new <Type>[<N>]` whose own
//! literal `N` falls outside `0..=128` is always flagged, regardless of
//! whether the resulting array's size ends up tracked: per the
//! CreationKit wiki's [Arrays
//! (Papyrus)](https://ck.uesp.net/wiki/Arrays_(Papyrus)) page, an array
//! created by a script via `New` (or grown with `Add()`) is hard-capped at
//! 128 elements by the engine. That page also notes this cap does *not*
//! apply to an array returned by a native function or to an
//! editor-populated array `Property` — such an array can legitimately hold
//! more than 128 elements — so an index into one of those is never flagged
//! against this limit, only against a locally tracked `new` size (above).

use std::collections::HashMap;

use papyrus_parser::ast::{AssignOp, BinaryOp, Expr, IfBranch, Literal, Stmt, UnaryOp};

use crate::none_form_usage::{all_functions, diverges};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "array-bounds";

/// The maximum number of elements an array created by a script (via `New`
/// or grown with `Add()`) can hold, per the CreationKit wiki's [Arrays
/// (Papyrus)](https://ck.uesp.net/wiki/Arrays_(Papyrus)) page. Does not
/// apply to an array returned by a native function or to an
/// editor-populated array `Property`, which this lint has no way to size
/// from source anyway.
const MAX_NEW_ARRAY_SIZE: i64 = 128;

/// Checks every function/event in `source` for a literal array index that
/// falls outside the constant size the array was declared with.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
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
            if let Some(literal_size) = eval_const_int(size) {
                if !(0..=MAX_NEW_ARRAY_SIZE).contains(&literal_size) {
                    diagnostics.push(Diagnostic {
                        line,
                        column: 1,
                        message: format!(
                            "[warning] Array size {literal_size} is outside the range Papyrus \
                             allows (0 to {MAX_NEW_ARRAY_SIZE}) for an array created with `new`"
                        ),
                        rule: RULE,
                    });
                }
            }
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
fn eval_const_int(expr: &Expr) -> Option<i64> {
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
mod tests {
    use super::*;

    #[test]
    fn flags_a_literal_index_past_the_declared_size() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float[] a = new Float[3]\n    a[5] = 0.1\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("'a'"));
        assert!(diagnostics[0].message.contains('5'));
        assert!(diagnostics[0].message.contains('3'));
    }

    #[test]
    fn flags_a_negative_literal_index() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[3]\n    a[-1] = 1\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn flags_an_out_of_bounds_read() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[2]\n    Int v = a[2]\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_flag_an_index_within_bounds() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float[] a = new Float[3]\n    a[0] = 0.1\n    a[2] = 0.2\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_index_at_the_last_valid_position() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[1]\n    a[0] = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_non_literal_index() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int i)\n    Int[] a = new Int[3]\n    a[i] = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_array_whose_size_is_not_a_literal() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int n)\n    Int[] a = new Int[n]\n    a[50] = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_unrelated_identifier() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test(Int[] a)\n    a[50] = 1\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn stops_tracking_a_variable_reassigned_to_something_else() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int[] other)\n    Int[] a = new Int[2]\n    a = other\n    a[50] = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn tracks_a_size_updated_by_a_later_reassignment() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[2]\n    a = new Int[5]\n    a[4] = 1\n    a[5] = 1\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 7);
    }

    #[test]
    fn constant_folding_handles_literal_arithmetic_in_the_size_and_index() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[1 + 2]\n    a[1 + 2] = 1\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_flag_after_both_if_branches_agree_on_the_size() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int[] a\n    If flag\n        a = new Int[3]\n    Else\n        a = new Int[3]\n    EndIf\n    a[2] = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_after_if_branches_disagree_on_the_size() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int[] a\n    If flag\n        a = new Int[3]\n    Else\n        a = new Int[5]\n    EndIf\n    a[4] = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_use_after_while_loop_since_it_may_run_zero_times() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int[] a = new Int[1]\n    While flag\n        a = new Int[5]\n        flag = false\n    EndWhile\n    a[4] = 1\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 9);
    }

    #[test]
    fn flags_a_new_array_larger_than_the_engine_maximum() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[200]\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.contains("200"));
        assert!(diagnostics[0].message.contains("128"));
    }

    #[test]
    fn flags_a_new_array_with_a_negative_size() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[-1]\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn does_not_flag_a_new_array_at_the_engine_maximum() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[128]\n    a[127] = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_indexing_a_property_array_beyond_128() {
        // Editor-populated array Properties (and arrays returned by native
        // functions) aren't subject to the 128-element `new`/`Add()` cap, and
        // this lint has no way to know such a property's actual configured
        // size from source, so an index like this is deliberately left
        // unflagged rather than risk a false positive.
        let diagnostics = check(
            "ScriptName Example\n\nFloat[] Property a Auto\n\nFloat Function DoA()\n    Return a[128]\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_call_arguments_and_nested_indices() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[2]\n    Int[] b = new Int[1]\n    Consume(a[5])\n    Int v = a[b[0]]\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Int[] a = new Int[1]\n        a[5] = 1\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn each_function_starts_fresh() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction First()\n    Int[] a = new Int[5]\nEndFunction\n\nFunction Second()\n    Int[] a = new Int[1]\n    a[4] = 1\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 9);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }
}
