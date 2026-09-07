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
//! to what follows the `If`; a variable assigned by any surviving branch
//! counts as assigned afterward too — the standard Papyrus idiom of
//! assigning a variable in only one branch (or omitting `Else` entirely)
//! and later testing whether that ran, by comparing it against its
//! default, would otherwise still be misread as a bug. Only a variable
//! left unassigned by every surviving branch stays flagged past the `If`.
//! A `While` loop may run zero times, so an assignment made only
//! inside its body is never assumed to have run by the time execution
//! reaches the code after the loop. Function parameters and script
//! properties always have a value by the time a function runs and are
//! never tracked by this lint.
//!
//! An `==`/`!=` comparison against the variable's own declared type's
//! implicit default (`None`, `0`, `0.0`, `False`, or `""`) is a deliberate
//! gate on "has this been set yet?" rather than a genuine read of the
//! value, so the compared variable is never flagged for that comparison
//! specifically (other reads of it elsewhere still are). The declared
//! type is tracked alongside each unassigned local so a mismatched
//! comparison — `Int i` against `None`, or `Bool b` against `0` — is never
//! mistaken for that type's own default and still gets flagged. That gate
//! also carries across a short-circuiting `&&`/`||` joining it to a further
//! operand, the same way [`crate::none_form_usage`] narrows its own
//! known-`None` state through those operators: `x == <default> || x.Foo()`
//! and `x != <default> && x.Foo()` both only evaluate `x.Foo()` once the
//! gate has established `x` is no longer at its default, so that operand is
//! never flagged either.

use std::collections::HashMap;

use papyrus_parser::ast::{AssignOp, BinaryOp, Expr, IfBranch, Literal, Stmt, TypeName};

use crate::none_form_usage::{all_functions, diverges};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "variable-used-before-assignment";

/// The implicit default value Papyrus gives an unassigned local, grouped by
/// which literal spells it: `Int`/`Float`/`Bool`/`String` locals default to
/// `0`/`0.0`/`False`/`""` respectively, while every other type (object
/// references and arrays alike) defaults to `None`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DefaultKind {
    Int,
    Float,
    Bool,
    String,
    None_,
}

impl DefaultKind {
    fn of(type_name: &TypeName) -> Self {
        if type_name.is_array {
            return DefaultKind::None_;
        }
        match type_name.name.to_lowercase().as_str() {
            "int" => DefaultKind::Int,
            "float" => DefaultKind::Float,
            "bool" => DefaultKind::Bool,
            "string" => DefaultKind::String,
            _ => DefaultKind::None_,
        }
    }

    /// Whether `literal` is this default's own spelling (as opposed to some
    /// other type's default, which a comparison happens to also use).
    fn matches(self, literal: &Literal) -> bool {
        match (self, literal) {
            (DefaultKind::Int, Literal::Int { value: 0, .. }) => true,
            (DefaultKind::Float, Literal::Float(value)) => *value == 0.0,
            (DefaultKind::Bool, Literal::Bool(false)) => true,
            (DefaultKind::String, Literal::String(value)) => value.is_empty(),
            (DefaultKind::None_, Literal::None) => true,
            _ => false,
        }
    }
}

/// Checks every function/event in `source` for a local variable read before
/// it's ever been assigned a value.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        let mut unassigned = HashMap::new();
        walk_body(&function.body, &mut unassigned, &mut diagnostics);
    }
    diagnostics
}

fn walk_body(
    body: &[Stmt],
    unassigned: &mut HashMap<String, DefaultKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    check_expr(value, unassigned, diagnostics, decl.line);
                } else {
                    unassigned.insert(decl.name.to_lowercase(), DefaultKind::of(&decl.type_name));
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

/// Handles an `If`/`ElseIf`/`Else` chain: each branch's own condition
/// narrows the incoming state before its body is walked (a default-value
/// gate that rules out the default, e.g. `found != None && ...`, means the
/// gated variable is no longer treated as unassigned for the rest of that
/// branch's body), and a single-branch `If`'s `Else` gets the opposite
/// narrowing from that same condition; only branches that don't
/// unconditionally `Return` contribute their exit state to what follows the
/// `If`. A variable assigned by *any* surviving branch is treated as
/// assigned afterward too, even though a branch that doesn't run it
/// wouldn't have — the standard Papyrus idiom is to assign a variable in
/// only one branch (or omit the `Else` entirely) and later test whether
/// that ran by comparing it against its default, so requiring every branch
/// to assign it before dropping the flag would keep flagging that idiom's
/// later reads as if they were bugs. Only a variable left unassigned by
/// *every* surviving branch is still flagged past the `If`.
fn handle_if(
    branches: &[IfBranch],
    else_body: &[Stmt],
    unassigned: &mut HashMap<String, DefaultKind>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_vars = unassigned.clone();
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

    *unassigned = if surviving.is_empty() {
        // Every branch (including the implicit/explicit else) returns, so
        // nothing after the `If` is reached through it; keep the pre-`If`
        // state rather than guess.
        entry_vars
    } else {
        let mut merged = surviving.swap_remove(0);
        for branch_vars in &surviving {
            merged.retain(|name, kind| branch_vars.get(name) == Some(kind));
        }
        merged
    };
}

/// Flags `name` if it's currently tracked as unassigned in `unassigned`.
fn check_identifier(
    name: &str,
    unassigned: &HashMap<String, DefaultKind>,
    diagnostics: &mut Vec<Diagnostic>,
    line: usize,
) {
    if unassigned.contains_key(&name.to_lowercase()) {
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
    unassigned: &HashMap<String, DefaultKind>,
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
        Expr::Binary {
            left,
            op: BinaryOp::And,
            right,
        } => {
            check_expr(left, unassigned, diagnostics, line);
            // Short-circuit: `right` only evaluates once `left` is truthy.
            let mut narrowed = unassigned.clone();
            narrow_for_truthy(left, &mut narrowed);
            check_expr(right, &narrowed, diagnostics, line);
        }
        Expr::Binary {
            left,
            op: BinaryOp::Or,
            right,
        } => {
            check_expr(left, unassigned, diagnostics, line);
            // Short-circuit: `right` only evaluates once `left` is falsy.
            let mut narrowed = unassigned.clone();
            narrow_for_falsy(left, &mut narrowed);
            check_expr(right, &narrowed, diagnostics, line);
        }
        Expr::Binary { left, op, right } => {
            let is_default_value_gate = matches!(op, BinaryOp::Eq | BinaryOp::NotEq)
                && default_value_gate(left, right, unassigned).is_some();
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

/// If `left`/`right` (in either order) is a plain identifier, still tracked
/// as unassigned, compared against the literal spelling of that
/// identifier's own declared-type default — the "has this been set yet?"
/// gate pattern this lint deliberately doesn't treat as a use of the
/// variable — returns that identifier's name (lowercased). A comparison
/// against some other type's default (`Int i` against `None`, `Bool b`
/// against `0`, ...) doesn't match and is still treated as a genuine read.
fn default_value_gate(
    left: &Expr,
    right: &Expr,
    unassigned: &HashMap<String, DefaultKind>,
) -> Option<String> {
    match (left, right) {
        (Expr::Identifier(name), Expr::Literal(lit))
        | (Expr::Literal(lit), Expr::Identifier(name)) => {
            let key = name.to_lowercase();
            unassigned
                .get(&key)
                .is_some_and(|kind| kind.matches(lit))
                .then_some(key)
        }
        _ => None,
    }
}

/// If `condition` is a direct default-value gate on some variable (`x ==
/// <its default>` or `x != <its default>`), returns its name (lowercased)
/// along with whether `condition` being *true* means that variable is
/// still at its default.
fn default_gate_check(
    condition: &Expr,
    unassigned: &HashMap<String, DefaultKind>,
) -> Option<(String, bool)> {
    match condition {
        Expr::Binary {
            left,
            op: BinaryOp::Eq,
            right,
        } => default_value_gate(left, right, unassigned).map(|name| (name, true)),
        Expr::Binary {
            left,
            op: BinaryOp::NotEq,
            right,
        } => default_value_gate(left, right, unassigned).map(|name| (name, false)),
        _ => None,
    }
}

/// Narrows `state` to reflect `condition` having evaluated `true`,
/// recursing into `&&` operands (both must hold) — mirrors
/// [`crate::none_form_usage::narrow_for_truthy`], but removes a variable
/// from "still unassigned" state instead of adding it to "known None"
/// state.
fn narrow_for_truthy(condition: &Expr, state: &mut HashMap<String, DefaultKind>) {
    if let Expr::Binary {
        left,
        op: BinaryOp::And,
        right,
    } = condition
    {
        narrow_for_truthy(left, state);
        narrow_for_truthy(right, state);
        return;
    }
    if let Some((name, means_default)) = default_gate_check(condition, state) {
        if !means_default {
            state.remove(&name);
        }
    }
}

/// Narrows `state` to reflect `condition` having evaluated `false`,
/// recursing into `||` operands (both must have been false) — mirrors
/// [`crate::none_form_usage::narrow_for_falsy`], but removes a variable
/// from "still unassigned" state instead of adding it to "known None"
/// state.
fn narrow_for_falsy(condition: &Expr, state: &mut HashMap<String, DefaultKind>) {
    if let Expr::Binary {
        left,
        op: BinaryOp::Or,
        right,
    } = condition
    {
        narrow_for_falsy(left, state);
        narrow_for_falsy(right, state);
        return;
    }
    if let Some((name, means_default)) = default_gate_check(condition, state) {
        if means_default {
            state.remove(&name);
        }
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
    fn does_not_flag_use_after_one_sided_if_assigns_it() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    If flag\n        i = 1\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_use_when_only_one_branch_of_several_assigns() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag, Bool other)\n    Int i\n    If flag\n        Debug.Trace(\"a\")\n    ElseIf other\n        i = 1\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn still_flags_use_after_if_else_when_neither_branch_assigns() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int i\n    If flag\n        Debug.Trace(\"a\")\n    Else\n        Debug.Trace(\"b\")\n    EndIf\n    Debug.Trace(i as String)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 10);
    }

    #[test]
    fn does_not_flag_the_conditional_assign_then_default_check_idiom() {
        // The reported false positive (papyrus-lint#363): a variable is
        // assigned in only one branch, then a *later*, separate `If`
        // compares it against its default to find out whether that branch
        // ran, deliberately relying on the language's implicit default.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag, Bool sigilStoneInstalled)\n    Bool daedricItemCrafted\n    If flag\n        daedricItemCrafted = True\n    EndIf\n    If sigilStoneInstalled == False || daedricItemCrafted == False\n        Debug.Trace(\"no recipes\")\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
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

    #[test]
    fn still_flags_an_int_compared_against_none_since_that_is_not_its_default() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    If i == None\n        i = 5\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn still_flags_a_bool_compared_against_zero_since_that_is_not_its_default() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Bool b\n    If b == 0\n        b = True\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_flag_an_or_joined_default_gate_guarding_a_while_condition() {
        // papyrus-lint#457: `a == None || a.IsDead()` only evaluates
        // `a.IsDead()` once the gate has established `a` is no longer at
        // its default, so it shouldn't be flagged as a read of `a` before
        // assignment.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    While a == None || a.IsDead()\n        a = Game.GetPlayer()\n    EndWhile\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_or_joined_default_gate_guarding_an_if_condition() {
        // Same shape as above, but in a plain `If` rather than a `While`
        // condition, confirming the fix isn't loop-specific.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    If a == None || a.IsDead()\n        a = Game.GetPlayer()\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_and_joined_default_gate_the_other_polarity() {
        // The `&&` polarity: `a != None && a.IsDead()` only evaluates
        // `a.IsDead()` once the gate has established `a` is no longer at
        // its default.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    If a != None && a.IsDead()\n        a = Game.GetPlayer()\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn still_flags_an_or_joined_operand_when_the_gate_does_not_rule_out_the_default() {
        // `a != None || a.IsDead()`: the gate being false (needed to reach
        // `a.IsDead()`) means `a == None`, so the default is *not* ruled
        // out and the read is still a genuine bug.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    If a != None || a.IsDead()\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn still_flags_an_and_joined_operand_when_the_gate_does_not_rule_out_the_default() {
        // `a == None && a.IsDead()`: the gate being true (needed to reach
        // `a.IsDead()`) means `a == None`, so the default is *not* ruled
        // out and the read is still a genuine bug.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    If a == None && a.IsDead()\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_flag_a_read_inside_an_and_joined_default_gate_branch_body() {
        // papyrus-lint: `found != None && !found.IsDead()` rules out
        // `found`'s default before the branch body runs, so reading
        // `found` inside that body (not just within the condition itself)
        // shouldn't be flagged either.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor found\n    If found != None && !found.IsDead()\n        Float new_x = found.GetPositionX()\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_read_in_the_else_body_of_a_single_default_gate_branch() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor found\n    If found != None\n        Debug.Trace(\"found\")\n    Else\n        found.GetPositionX()\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 8);
    }

    #[test]
    fn does_not_flag_a_chain_of_or_joined_default_gates_before_a_final_read() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    Actor b\n    If a == None || b == None || a.IsDead()\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }
}
