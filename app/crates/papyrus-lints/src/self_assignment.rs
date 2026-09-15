//! Flags a plain `=` assignment whose right-hand side is the exact same
//! reference as its own target (e.g. `a = a`, `Self.Foo = Self.Foo`,
//! `akRef.Foo = akRef.Foo`), since that assignment can never change the
//! value it reads and is almost always a copy-paste mistake or leftover
//! from a refactor.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to compare the target and value expressions structurally; a script
//! that doesn't parse cleanly is left unchecked rather than guessed at.
//!
//! Only a bare identifier or a chain of member accesses rooted at one (or
//! at `Self`) is ever compared this way — a call, an index, or any other
//! expression shape never counts as a self-assignment, since re-evaluating
//! it on both sides of the same line isn't guaranteed to read the same
//! value twice (or may have side effects of its own). A compound
//! assignment (`+=`, `-=`, ...) is never flagged either, since unlike
//! plain `=` it does change the target's value.

use papyrus_parser::ast::{AssignOp, Expr, FunctionDecl, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "self-assignment";

/// Checks `source` for a plain `=` assignment whose target and value are the
/// exact same simple reference. Flagged as a `[warning]`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        for assign in collect_assigns(&function.body) {
            let Stmt::Assign {
                target,
                op: AssignOp::Assign,
                value,
                line,
            } = assign
            else {
                continue;
            };
            let (Some(target_key), Some(value_key)) = (reference_key(target), reference_key(value))
            else {
                continue;
            };
            if target_key != value_key {
                continue;
            }

            diagnostics.push(Diagnostic {
                line: *line,
                column: 1,
                message: "[warning] This assigns a value to itself, which has no effect"
                    .to_string(),
                rule: RULE,
            });
        }
    }
    diagnostics
}

/// A canonical, case-insensitive key identifying a "simple reference"
/// expression (a bare identifier, `Self`, or a chain of member accesses
/// rooted at either), or `None` for any other expression shape (a call, an
/// index, a literal, ...), which is never compared for self-assignment.
fn reference_key(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(name) => Some(name.to_ascii_lowercase()),
        Expr::Self_ => Some("self".to_string()),
        Expr::Member { object, property } => Some(format!(
            "{}.{}",
            reference_key(object)?,
            property.to_ascii_lowercase()
        )),
        _ => None,
    }
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

/// Finds every `Assign` statement in `body`, including ones nested inside
/// `If`/`ElseIf`/`Else` branches and `While` bodies.
fn collect_assigns(body: &[Stmt]) -> Vec<&Stmt> {
    let mut assigns = Vec::new();
    for stmt in body {
        match stmt {
            Stmt::Assign { .. } => assigns.push(stmt),
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    assigns.extend(collect_assigns(&branch.body));
                }
                assigns.extend(collect_assigns(else_body));
            }
            Stmt::While { body, .. } => assigns.extend(collect_assigns(body)),
            _ => {}
        }
    }
    assigns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_local_variable_assigned_to_itself() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 10\n    a = a\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
    }

    #[test]
    fn matches_the_name_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 10\n    a = A\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_self_qualified_property_assigned_to_itself() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property a Auto\n\nFunction Test()\n    Self.a = Self.a\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_matching_member_chain_assigned_to_itself() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(SomeQuest akQuest)\n    akQuest.Stage = akQuest.Stage\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_a_bare_name_assigned_to_a_self_qualified_one() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property a Auto\n\nFunction Test()\n    a = Self.a\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_compound_assignment() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 10\n    a += a\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_assignment_of_a_different_variable() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 10\n    Int b = 5\n    a = b\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_call_even_when_written_identically_on_both_sides() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(SomeQuest akQuest)\n    Int a = akQuest.GetStage()\n    a = akQuest.GetStage()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_index_expression() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[3]\n    a[0] = a[0]\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_assignment_inside_if_block() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 10\n    If true\n        a = a\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn flags_assignment_inside_while_loop() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 10\n    While a > 0\n        a = a\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Waiting\n    Function Test()\n        Int a = 10\n        a = a\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn returns_no_diagnostics_for_a_script_that_fails_to_parse() {
        let diagnostics = check("ScriptName Example\n\nFunction Test(\n    a = a\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }
}
