//! Flags a function/event that calls itself with no conditional logic
//! anywhere in its body that could ever stop it from doing so again on the
//! next call, since that's an unconditional infinite recursion that will
//! exhaust the call stack.
//!
//! This is deliberately very conservative: it never follows a call chain
//! through another function (that's out of scope entirely), and within one
//! function it only follows statements that are guaranteed to execute. An
//! `If True` body is guaranteed, so it does not hide an otherwise unguarded
//! self-call; any runtime-dependent `If` or any `While` still disqualifies
//! the function because it could act as a base case or skip the call. The
//! much more common recursion pattern, an `If` guarding an early
//! `Return` before the recursive call, is left completely alone by this
//! rule (any runtime-dependent `If` or `While` in the function disqualifies
//! it), even though that guard might not actually cover every case — proving
//! that would mean evaluating the condition, which this lint doesn't attempt.
//! The goal is to catch the plain "this function just calls itself,
//! unconditionally, every single time" mistake with no false positives, not
//! to reason about whether a given guard is correct.
//!
//! A self-call reached only through the right-hand side of a short-circuit
//! `&&`/`\|\|` is not considered unconditional either, since that side may
//! never actually evaluate.

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
        find_self_calls_in_body(&function.body, &name_lower, &mut diagnostics);
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

/// Whether `body` contains runtime-dependent branching. An `If True` selects
/// its first body unconditionally, so only branching inside that body matters.
fn has_branching(body: &[Stmt]) -> bool {
    body.iter().any(|stmt| match stmt {
        Stmt::If { branches, .. }
            if matches!(
                branches.first().map(|branch| &branch.condition),
                Some(Expr::Literal(Literal::Bool(true)))
            ) =>
        {
            has_branching(&branches[0].body)
        }
        Stmt::If { .. } | Stmt::While { .. } => true,
        _ => false,
    })
}

fn find_self_calls_in_body(body: &[Stmt], name_lower: &str, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    find_self_calls(value, name_lower, diagnostics);
                }
            }
            Stmt::Assign { target, value, .. } => {
                find_self_calls(target, name_lower, diagnostics);
                find_self_calls(value, name_lower, diagnostics);
            }
            Stmt::Expr { value, .. } => find_self_calls(value, name_lower, diagnostics),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    find_self_calls(value, name_lower, diagnostics);
                }
            }
            Stmt::If { branches, .. } => {
                find_self_calls_in_body(&branches[0].body, name_lower, diagnostics);
            }
            Stmt::While { .. } => unreachable!("branching bodies are filtered before scanning"),
        }
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
    fn flags_a_self_call_inside_an_always_true_if() {
        let source =
            "ScriptName Example\n\nFunction A()\n  If True\n    A()\n  EndIf\nEndFunction\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert_eq!(diagnostics[0].rule, RULE);
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
}
