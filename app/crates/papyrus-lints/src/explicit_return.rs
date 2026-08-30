//! Flags a typed function/event whose control flow can reach the end of its
//! body without hitting a `Return` statement, since Papyrus then silently
//! returns that type's default value (`0`, `""`, `False`, or `None`)
//! instead of a value the author actually chose.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! the block structure of the function body; a script that doesn't parse
//! cleanly is left unchecked rather than guessed at. A function with no
//! declared return type isn't checked (falling off its end is the normal,
//! intended way for it to finish). A `While` loop is never assumed to
//! guarantee a `Return`, since it may run zero times; an `If` only
//! guarantees one when every branch (`If`/`ElseIf`, and an `Else`) does, so
//! an `If` with no `Else` never counts, matching the fact that its
//! condition might not match any branch at runtime. A native function
//! (no body to inspect) is never flagged.

use papyrus_parser::ast::{FunctionDecl, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "explicit-return";

/// Checks `source` for typed functions/events with a code path that falls
/// off the end of the body without an explicit `Return`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    all_functions(&script)
        .filter(|function| function.return_type.is_some())
        .filter(|function| !function.is_native)
        .filter(|function| !body_always_returns(&function.body))
        .map(|function| Diagnostic {
            line: function.line,
            column: 1,
            message: format!(
                "[error] Function '{}' does not return a value on every code path",
                function.name
            ),
            rule: RULE,
        })
        .collect()
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

/// Whether every path through `body` is guaranteed to execute a `Return`
/// before falling off the end of the block.
fn body_always_returns(body: &[Stmt]) -> bool {
    body.iter().any(stmt_always_returns)
}

fn stmt_always_returns(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return { .. } => true,
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            body_always_returns(else_body)
                && branches
                    .iter()
                    .all(|branch| body_always_returns(&branch.body))
        }
        Stmt::While { .. } | Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_typed_function_with_no_return_at_all() {
        let diagnostics =
            check("ScriptName Example\n\nInt Function Test()\n    Int i = 1\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert!(diagnostics[0].message.contains("'Test'"));
        assert!(diagnostics[0].message.starts_with("[error]"));
        assert_eq!(diagnostics[0].rule, RULE);
    }

    #[test]
    fn allows_a_trailing_unconditional_return() {
        let diagnostics =
            check("ScriptName Example\n\nInt Function Test()\n    Return 1\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_function_with_no_declared_return_type() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_native_functions() {
        let diagnostics = check("ScriptName Example\n\nInt Function Test() Native\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_an_if_with_no_else() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    If flag\n        Return 1\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_an_if_else_where_only_one_branch_returns() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    If flag\n        Return 1\n    Else\n        Int i = 1\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn allows_an_if_else_where_every_branch_returns() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    If flag\n        Return 1\n    Else\n        Return 2\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn allows_an_if_elseif_else_where_every_branch_returns() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Int n)\n    If n == 1\n        Return 1\n    ElseIf n == 2\n        Return 2\n    Else\n        Return 3\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_an_if_elseif_else_where_one_elseif_branch_falls_through() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Int n)\n    If n == 1\n        Return 1\n    ElseIf n == 2\n        Int i = 1\n    Else\n        Return 3\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn a_return_after_the_if_still_covers_a_non_exhaustive_if() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    If flag\n        Return 1\n    EndIf\n    Return 2\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_treat_a_while_loop_as_a_guaranteed_return() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    While flag\n        Return 1\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn allows_a_bare_return_in_a_typed_function() {
        // A bare `Return` is still an explicit return statement on this
        // path; whether it carries a value matching the declared return
        // type is `return_types`' concern, not this lint's.
        let diagnostics =
            check("ScriptName Example\n\nInt Function Test()\n    Return\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Int Function Test()\n        Int i = 1\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nInt Function Test(\nEndFunction\n").is_empty());
    }

    #[test]
    fn nested_if_inside_while_still_requires_a_trailing_return() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    While flag\n        If flag\n            Return 1\n        Else\n            Return 2\n        EndIf\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }
}
