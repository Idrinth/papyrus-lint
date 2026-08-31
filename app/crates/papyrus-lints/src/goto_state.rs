//! Flags a `GoToState("Name")` call whose target state can't be found,
//! since a typo'd or renamed state name compiles fine but silently never
//! takes effect: the engine just falls back through the state resolution
//! algorithm instead of raising an error (see
//! <https://ck.uesp.net/wiki/State_Reference>).
//!
//! Per that same reference, a `GoToState` target does *not* have to exist
//! on the script it's called from — it may only be declared on a script
//! that extends this one, which is a legitimate way to forward-declare a
//! state for an as-yet-unwritten child script to implement. To keep that
//! pattern from being flagged, a target not declared on this script is
//! only reported when this script has no `Extends` at all (nothing else
//! could ever define it) or, once resolved through `external`'s knowledge
//! of the project (see [`check_with`]), when it isn't declared anywhere in
//! this script's own ancestry either. The empty string (`GoToState("")`,
//! switching back to the empty state) is always valid and never flagged.

use std::collections::HashSet;

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Literal, Script, Stmt};

use crate::argument_types::{ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "goto-state";

/// Checks `source` for `GoToState` calls whose target state can't be found
/// on the script itself. A script that `Extends` another is left
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
    match expr {
        Expr::Call {
            callee,
            args,
            line,
            col,
        } => {
            if is_goto_state_callee(callee) {
                if let [Expr::Literal(Literal::String(name))] = args.as_slice() {
                    if is_missing(name, script, local_states, external) {
                        diagnostics.push(missing(*line, *col, name));
                    }
                }
            }
            walk_expr(callee, script, local_states, external, diagnostics);
            for arg in args {
                walk_expr(arg, script, local_states, external, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, script, local_states, external, diagnostics);
            walk_expr(right, script, local_states, external, diagnostics);
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
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Whether `callee` is a bare `GoToState(...)` call, or one explicitly
/// qualified with `self.GoToState(...)`. `GoToState` always acts on the
/// script it's called from, so no other qualifier is recognized.
fn is_goto_state_callee(callee: &Expr) -> bool {
    match callee {
        Expr::Identifier(name) => name.eq_ignore_ascii_case("GoToState"),
        Expr::Member { object, property } => {
            matches!(**object, Expr::Self_) && property.eq_ignore_ascii_case("GoToState")
        }
        _ => false,
    }
}

/// Whether `name` can't be resolved as a state this script switches
/// into: not the empty string, not declared locally, and — when this
/// script `Extends` another — not found in that ancestry either (per
/// `external`; see the module docs).
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
        message: format!("[warning] GoToState references state '{name}', which could not be found"),
        rule: RULE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_goto_state_call_to_an_undeclared_state() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    GoToState(\"Missing\")\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'Missing'"));
        assert_eq!(diagnostics[0].rule, RULE);
    }

    #[test]
    fn does_not_flag_a_call_to_a_state_declared_locally() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\nEndState\n\nFunction Test()\n    GoToState(\"Active\")\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn matches_the_state_name_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\nEndState\n\nFunction Test()\n    GoToState(\"active\")\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_switching_to_the_empty_state() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    GoToState(\"\")\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_self_qualified_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    self.GoToState(\"Missing\")\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_a_call_through_another_object() {
        // GoToState always acts on `self`; a call qualified by something
        // else is a different function entirely (or invalid), not this
        // lint's concern.
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Example akOther)\n    akOther.GoToState(\"Missing\")\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn skips_a_non_literal_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(String stateName)\n    GoToState(stateName)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_undeclared_target_without_a_resolver_when_extending_another_script() {
        // The target might be declared on a script further up `Extends`
        // that this crate can't resolve on its own; see `check_with`.
        let diagnostics = check(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    GoToState(\"Missing\")\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_calls_in_nested_control_flow() {
        let diagnostics = check(
            r#"
ScriptName Example

Function Test(Bool condition)
    If condition
        GoToState("Missing")
    EndIf
EndFunction
"#,
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn checks_calls_inside_state_functions_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        GoToState(\"Missing\")\n    EndFunction\nEndState\n",
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
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    GoToState(\"FromParent\")\nEndFunction\n",
            &mut FakeExternalWithAncestorState,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_target_not_found_anywhere_in_the_extends_ancestry() {
        let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    GoToState(\"StillMissing\")\nEndFunction\n",
            &mut FakeExternalWithAncestorState,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'StillMissing'"));
    }
}
