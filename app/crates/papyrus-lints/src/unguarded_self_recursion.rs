//! Flags a function/event that calls itself with no conditional logic
//! anywhere in its body that could ever stop it from doing so again on the
//! next call, since that's an unconditional infinite recursion that will
//! exhaust the call stack.
//!
//! This is deliberately very conservative: it never follows a call chain
//! through another function (that's out of scope entirely), and within one
//! function it only fires when the *entire* function body contains no `If`
//! or `While` at all — i.e. there is no branch or loop anywhere that could
//! possibly act as a base case or otherwise skip the recursive call on some
//! call. The much more common recursion pattern, an `If` guarding an early
//! `Return` before the recursive call, is left completely alone by this
//! rule (any `If`/`While` in the function disqualifies it), even though
//! that guard might not actually cover every case — proving that would mean
//! evaluating the condition, which this lint doesn't attempt. The goal is
//! to catch the plain "this function just calls itself, unconditionally,
//! every single time" mistake with no false positives, not to reason about
//! whether a given guard is correct.
//!
//! A self-call reached only through the right-hand side of a short-circuit
//! `&&`/`\|\|` is not considered unconditional either, since that side may
//! never actually evaluate.
//!
//! A `GoToState(...)` call to a different state also acts as a guard for any
//! self-call that follows it in the same (necessarily linear, per
//! [`has_branching`]) body, but only when that other state actually declares
//! its own handler for this function/event: Bethesda's standard
//! save-compatibility idiom re-points event dispatch at another state right
//! before calling itself again, so the "recursive" call never actually
//! re-enters this body — it dispatches into the new state's (typically
//! empty) handler instead. If the target state declares no such handler,
//! dispatch falls back to the empty state's own declaration, which may still
//! be this exact function, so the call is left flagged in that case.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, Literal, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unguarded-self-recursion";

/// Checks every function/event declared in `source` for an unconditional
/// self-call, per the module documentation above.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        if function.is_native || has_branching(&function.body) {
            continue;
        }
        let name_lower = function.name.to_lowercase();
        let current_state_lower = function.state.as_deref().unwrap_or("").to_lowercase();
        let mut guarded_by_goto_state = false;
        for stmt in &function.body {
            if !guarded_by_goto_state {
                for expr in stmt_exprs(stmt) {
                    find_self_calls(expr, &name_lower, &mut diagnostics);
                }
            }
            if let Stmt::Expr { value, .. } = stmt {
                if let Some(target_state) = goto_state_target(value) {
                    if !target_state.eq_ignore_ascii_case(&current_state_lower)
                        && state_has_handler(&script, target_state, &name_lower)
                    {
                        guarded_by_goto_state = true;
                    }
                }
            }
        }
    }
    diagnostics
}

/// Whether `expr` is a call to `GoToState("SomeState")` (a bare call, or
/// `Self.GoToState(...)`), and if so, the state name it targets.
fn goto_state_target(expr: &Expr) -> Option<&str> {
    let Expr::Call { callee, args, .. } = expr else {
        return None;
    };
    let is_goto_state = match &**callee {
        Expr::Identifier(name) => name.eq_ignore_ascii_case("GoToState"),
        Expr::Member { object, property } => {
            matches!(**object, Expr::Self_) && property.eq_ignore_ascii_case("GoToState")
        }
        _ => false,
    };
    if !is_goto_state {
        return None;
    }
    match args.first() {
        Some(Expr::Literal(Literal::String(state_name))) => Some(state_name.as_str()),
        _ => None,
    }
}

/// Whether `script` declares a `State target_state_name` block containing a
/// function/event named `function_name_lower` (matched case-insensitively),
/// i.e. whether switching into that state would actually dispatch calls to
/// `function_name_lower` somewhere other than the empty state's declaration.
fn state_has_handler(script: &Script, target_state_name: &str, function_name_lower: &str) -> bool {
    script.states.iter().any(|state| {
        state.name.eq_ignore_ascii_case(target_state_name)
            && state
                .functions
                .iter()
                .any(|f| f.name.to_lowercase() == function_name_lower)
    })
}

fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

/// Whether `body` contains an `If` or `While` anywhere, which disqualifies
/// the whole function from this lint: this language has no other way to
/// conditionally skip code, so their absence means every statement in
/// `body` always runs, in order, on every call.
fn has_branching(body: &[Stmt]) -> bool {
    body.iter()
        .any(|stmt| matches!(stmt, Stmt::If { .. } | Stmt::While { .. }))
}

/// The expression(s) a top-level statement evaluates, in the order they
/// evaluate, for [`find_self_calls`] to search. `If`/`While` never appear
/// here since [`has_branching`] already excludes any function containing
/// one before this is ever called.
fn stmt_exprs(stmt: &Stmt) -> Vec<&Expr> {
    match stmt {
        Stmt::VarDecl(decl) => decl.value.iter().collect(),
        Stmt::Assign { target, value, .. } => vec![target, value],
        Stmt::Expr { value, .. } => vec![value],
        Stmt::Return { value, .. } => value.iter().collect(),
        Stmt::If { .. } | Stmt::While { .. } => Vec::new(),
    }
}

/// Recurses through `expr` looking for a call whose target resolves to the
/// function's own name (a bare identifier call, or `Self.Name(...)`),
/// pushing a diagnostic at each one found. Skips the right-hand side of a
/// short-circuiting `&&`/`\|\|`, since that side isn't guaranteed to
/// evaluate.
fn find_self_calls(expr: &Expr, name_lower: &str, diagnostics: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Call {
            callee,
            args,
            line,
            col,
        } => {
            let is_self_call = match &**callee {
                Expr::Identifier(name) => name.to_lowercase() == name_lower,
                Expr::Member { object, property } => {
                    matches!(**object, Expr::Self_) && property.to_lowercase() == name_lower
                }
                _ => false,
            };
            if is_self_call {
                diagnostics.push(Diagnostic {
                    line: *line,
                    column: *col,
                    message: "[warning] Unguarded recursion: this function calls itself with no \
                              conditional logic anywhere in its body that could ever prevent \
                              that call, so every invocation recurses forever until the call \
                              stack is exhausted"
                        .to_string(),
                    rule: RULE,
                });
            }
            find_self_calls(callee, name_lower, diagnostics);
            for arg in args {
                find_self_calls(arg, name_lower, diagnostics);
            }
        }
        Expr::NamedArg { value, .. } => find_self_calls(value, name_lower, diagnostics),
        Expr::Binary { left, op, right } => {
            find_self_calls(left, name_lower, diagnostics);
            if !matches!(op, BinaryOp::And | BinaryOp::Or) {
                find_self_calls(right, name_lower, diagnostics);
            }
        }
        Expr::Unary { operand, .. } => find_self_calls(operand, name_lower, diagnostics),
        Expr::Member { object, .. } => find_self_calls(object, name_lower, diagnostics),
        Expr::Index { object, index } => {
            find_self_calls(object, name_lower, diagnostics);
            find_self_calls(index, name_lower, diagnostics);
        }
        Expr::Cast { value, .. } => find_self_calls(value, name_lower, diagnostics),
        Expr::NewArray { size, .. } => find_self_calls(size, name_lower, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_function_that_unconditionally_calls_itself() {
        let source = "ScriptName Example\n\nFunction Foo()\n    Foo()\nEndFunction\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
    }

    #[test]
    fn flags_regardless_of_argument_values_and_surrounding_statements() {
        let source =
            "ScriptName Example\n\nFunction Foo(Int x)\n    Debug.Trace(\"x\")\n    Foo(x - 1)\nEndFunction\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn flags_a_self_call_reached_through_self() {
        let source = "ScriptName Example\n\nFunction Foo()\n    Self.Foo()\nEndFunction\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_self_call_used_as_a_return_value() {
        let source = "ScriptName Example\n\nInt Function Foo()\n    Return Foo()\nEndFunction\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_self_call_nested_in_an_assignments_value() {
        let source =
            "ScriptName Example\n\nInt Function Foo()\n    Int x = 1 + Foo()\n    Return x\nEndFunction\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn matches_the_function_name_case_insensitively() {
        let source = "ScriptName Example\n\nFunction Foo()\n    FOO()\nEndFunction\n";

        assert_eq!(check(source).len(), 1);
    }

    #[test]
    fn does_not_flag_the_classic_if_return_guarded_base_case() {
        let source = "ScriptName Example\n\nFunction Foo(Int x)\n    If x <= 0\n        Return\n    EndIf\n    Foo(x - 1)\nEndFunction\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn does_not_flag_recursion_inside_a_while_loop() {
        let source =
            "ScriptName Example\n\nFunction Foo(Int x)\n    While x > 0\n        Foo(x - 1)\n    EndWhile\nEndFunction\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn does_not_flag_a_call_to_a_different_function() {
        let source =
            "ScriptName Example\n\nFunction Foo()\n    Bar()\nEndFunction\n\nFunction Bar()\nEndFunction\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn does_not_flag_a_function_with_no_self_call() {
        let source = "ScriptName Example\n\nFunction Foo()\n    Debug.Trace(\"hi\")\nEndFunction\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn does_not_flag_a_self_call_gated_by_short_circuit_and() {
        let source =
            "ScriptName Example\n\nBool Function Foo(Bool cond)\n    Return cond && Foo(false)\nEndFunction\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn does_not_flag_a_native_function() {
        let source = "ScriptName Example\n\nFunction Foo() Native\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let source =
            "ScriptName Example\n\nState Active\n    Function Foo()\n        Foo()\n    EndFunction\nEndState\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Foo(\nEndFunction\n").is_empty());
    }

    #[test]
    fn does_not_flag_the_gotostate_save_compat_idiom() {
        let source = "ScriptName Example\n\nState Done\n    Event OnActivate(ObjectReference akActivator)\n    EndEvent\nEndState\n\nEvent OnActivate(ObjectReference akActivator)\n    GoToState(\"Done\")\n    OnActivate(akActivator)\nEndEvent\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn does_not_flag_gotostate_called_through_self() {
        let source = "ScriptName Example\n\nState Done\n    Event OnActivate(ObjectReference akActivator)\n    EndEvent\nEndState\n\nEvent OnActivate(ObjectReference akActivator)\n    Self.GoToState(\"Done\")\n    OnActivate(akActivator)\nEndEvent\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn still_flags_when_the_target_state_declares_no_matching_handler() {
        let source = "ScriptName Example\n\nState Done\n    Event OnLoad()\n    EndEvent\nEndState\n\nEvent OnActivate(ObjectReference akActivator)\n    GoToState(\"Done\")\n    OnActivate(akActivator)\nEndEvent\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 10);
    }

    #[test]
    fn still_flags_when_gotostate_targets_the_functions_own_state() {
        let source = "ScriptName Example\n\nState Active\n    Event OnActivate(ObjectReference akActivator)\n        GoToState(\"Active\")\n        OnActivate(akActivator)\n    EndEvent\nEndState\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn still_flags_a_self_call_preceding_a_gotostate_that_would_otherwise_guard_it() {
        let source = "ScriptName Example\n\nState Done\n    Event OnActivate(ObjectReference akActivator)\n    EndEvent\nEndState\n\nEvent OnActivate(ObjectReference akActivator)\n    OnActivate(akActivator)\n    GoToState(\"Done\")\nEndEvent\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 9);
    }
}
