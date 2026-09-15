//! Flags a `GetState() == "Name"`/`GetState() != "Name"` comparison (bare
//! `GetState()` or `self.GetState()`) whose named state can't be found,
//! since a typo'd or renamed state name still compiles fine but the
//! comparison then silently, permanently evaluates the opposite of what was
//! intended — a check against a state that can never be the current one is
//! always `false`, and its negation always `true` — instead of raising an
//! error.
//!
//! Like [`crate::goto_state`], a target not declared on this script isn't
//! flagged when this script `Extends` another and the target can't be ruled
//! out there either (see [`check_with`]) — it may only be declared on a
//! script further up that (unresolved) `Extends` chain, a legitimate way
//! for this script to compare against a state a not-yet-written child
//! implements. The empty string (`GetState() == ""`, checking whether the
//! script is currently in the empty state) is always valid and never
//! flagged.

use std::collections::HashSet;

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt};

use crate::argument_types::{ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "get-state-comparison";

/// Checks `source` for `GetState()` comparisons against a state that can't
/// be found on the script itself. A script that `Extends` another is left
/// unchecked when the target isn't declared locally, since it may be
/// declared further up that (unresolved) ancestry; see [`check_with`] to
/// resolve that too.
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but resolves a target not declared on the script itself
/// through `external`'s knowledge of the script's `Extends` ancestry,
/// flagging a target that can't be found there either.
pub fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let local_states: HashSet<String> = script
        .states
        .iter()
        .map(|state| state.name.to_ascii_lowercase())
        .collect();

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        for stmt in &function.body {
            walk_stmt(stmt, &script, &local_states, external, &mut diagnostics);
        }
    }
    diagnostics
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

fn walk_stmt<E: ExternalSignatures>(
    stmt: &Stmt,
    script: &Script,
    local_states: &HashSet<String>,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr(value, script, local_states, external, diagnostics);
            }
        }
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, script, local_states, external, diagnostics);
            walk_expr(value, script, local_states, external, diagnostics);
        }
        Stmt::Expr { value, .. } => walk_expr(value, script, local_states, external, diagnostics),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr(value, script, local_states, external, diagnostics);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for IfBranch {
                condition, body, ..
            } in branches
            {
                walk_expr(condition, script, local_states, external, diagnostics);
                for stmt in body {
                    walk_stmt(stmt, script, local_states, external, diagnostics);
                }
            }
            for stmt in else_body {
                walk_stmt(stmt, script, local_states, external, diagnostics);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, script, local_states, external, diagnostics);
            for stmt in body {
                walk_stmt(stmt, script, local_states, external, diagnostics);
            }
        }
    }
}

fn walk_expr<E: ExternalSignatures>(
    expr: &Expr,
    script: &Script,
    local_states: &HashSet<String>,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Expr::Binary { left, op, right } = expr {
        if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
            check_comparison(left, right, script, local_states, external, diagnostics);
        }
        walk_expr(left, script, local_states, external, diagnostics);
        walk_expr(right, script, local_states, external, diagnostics);
        return;
    }

    match expr {
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, script, local_states, external, diagnostics);
            for arg in args {
                walk_expr(arg, script, local_states, external, diagnostics);
            }
        }
        Expr::Unary { operand, .. } => {
            walk_expr(operand, script, local_states, external, diagnostics)
        }
        Expr::Member { object, .. } => {
            walk_expr(object, script, local_states, external, diagnostics)
        }
        Expr::Index { object, index } => {
            walk_expr(object, script, local_states, external, diagnostics);
            walk_expr(index, script, local_states, external, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, script, local_states, external, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, script, local_states, external, diagnostics),
        Expr::NamedArg { value, .. } => {
            walk_expr(value, script, local_states, external, diagnostics)
        }
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::Binary { .. } => {}
    }
}

/// Flags `left op right` when exactly one side is a bare/`self`-qualified
/// `GetState()` call (with no arguments) and the other is a string literal
/// naming a state that can't be found.
fn check_comparison<E: ExternalSignatures>(
    left: &Expr,
    right: &Expr,
    script: &Script,
    local_states: &HashSet<String>,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let target = get_state_call(left)
        .zip(string_literal(right))
        .or_else(|| get_state_call(right).zip(string_literal(left)));

    if let Some(((line, col), name)) = target {
        if is_missing(name, script, local_states, external) {
            diagnostics.push(missing(line, col, name));
        }
    }
}

fn string_literal(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Literal(Literal::String(name)) => Some(name.as_str()),
        _ => None,
    }
}

/// Returns the line/column of `expr` if it's a bare/`self`-qualified
/// `GetState()` call taking no arguments.
fn get_state_call(expr: &Expr) -> Option<(usize, usize)> {
    match expr {
        Expr::Call {
            callee,
            args,
            line,
            col,
        } if args.is_empty() && is_get_state_callee(callee) => Some((*line, *col)),
        _ => None,
    }
}

/// Whether `callee` is a bare `GetState(...)` call, or one explicitly
/// qualified with `self.GetState(...)`. `GetState` always reports the
/// state of the script it's called from, so no other qualifier is
/// recognized.
fn is_get_state_callee(callee: &Expr) -> bool {
    match callee {
        Expr::Identifier(name) => name.eq_ignore_ascii_case("GetState"),
        Expr::Member { object, property } => {
            matches!(**object, Expr::Self_) && property.eq_ignore_ascii_case("GetState")
        }
        _ => false,
    }
}

/// Whether `name` can't be resolved as a state this script could ever be
/// in: not the empty string, not declared locally, and — when this script
/// `Extends` another — not found in that ancestry either (per `external`;
/// see the module docs).
fn is_missing<E: ExternalSignatures>(
    name: &str,
    script: &Script,
    local_states: &HashSet<String>,
    external: &mut E,
) -> bool {
    if name.is_empty() || local_states.contains(&name.to_ascii_lowercase()) {
        return false;
    }
    match &script.extends {
        None => true,
        Some(parent) => !external.has_state(parent, name),
    }
}

fn missing(line: usize, col: usize, name: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!(
            "[error] GetState() compared against state '{name}', which could not be found"
        ),
        rule: RULE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_comparison_against_an_undeclared_state() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If GetState() == \"Missing\"\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.starts_with("[error]"));
        assert!(diagnostics[0].message.contains("'Missing'"));
        assert_eq!(diagnostics[0].rule, RULE);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn flags_a_not_equal_comparison_too() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If GetState() != \"Missing\"\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_the_literal_on_the_left_hand_side_too() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If \"Missing\" == GetState()\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_a_comparison_against_a_state_declared_locally() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\nEndState\n\nFunction Test()\n    If GetState() == \"Active\"\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn matches_the_state_name_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\nEndState\n\nFunction Test()\n    If GetState() == \"active\"\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_comparison_against_the_empty_state() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If GetState() == \"\"\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_self_qualified_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If self.GetState() == \"Missing\"\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_a_call_through_another_object() {
        // GetState always acts on `self`; a call qualified by something
        // else is a different function entirely (or invalid), not this
        // lint's concern.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Example akOther)\n    If akOther.GetState() == \"Missing\"\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn skips_a_non_literal_comparison() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(String stateName)\n    If GetState() == stateName\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_unrelated_equality_comparison() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_undeclared_target_without_a_resolver_when_extending_another_script() {
        // The target might be declared on a script further up `Extends`
        // that this crate can't resolve on its own; see `check_with`.
        let diagnostics = check(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    If GetState() == \"Missing\"\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_comparisons_in_nested_control_flow() {
        let diagnostics = check(
            r#"
ScriptName Example

Function Test(Bool condition)
    If condition
        If GetState() == "Missing"
        EndIf
    EndIf
EndFunction
"#,
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn checks_comparisons_inside_state_functions_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        If GetState() == \"Missing\"\n        EndIf\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    struct FakeExternalWithAncestorState;

    impl ExternalSignatures for FakeExternalWithAncestorState {
        fn lookup(
            &mut self,
            _type_name: &str,
            _function_name: &str,
        ) -> Option<Vec<crate::argument_types::ParamInfo>> {
            None
        }

        fn has_state(&mut self, type_name: &str, state_name: &str) -> bool {
            type_name.eq_ignore_ascii_case("BaseScript")
                && state_name.eq_ignore_ascii_case("FromParent")
        }
    }

    #[test]
    fn does_not_flag_a_target_resolved_through_the_extends_ancestry() {
        let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    If GetState() == \"FromParent\"\n    EndIf\nEndFunction\n",
            &mut FakeExternalWithAncestorState,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_target_not_found_anywhere_in_the_extends_ancestry() {
        let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    If GetState() == \"StillMissing\"\n    EndIf\nEndFunction\n",
            &mut FakeExternalWithAncestorState,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'StillMissing'"));
    }

    #[test]
    fn flags_a_comparison_nested_in_a_call_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    SomeCall(flag = GetState() == \"Missing\")\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'Missing'"));
    }

    #[test]
    fn walks_an_indexed_argument_without_crashing() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(String[] names, Int i)\n    Debug.Trace(names[i])\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }
}
