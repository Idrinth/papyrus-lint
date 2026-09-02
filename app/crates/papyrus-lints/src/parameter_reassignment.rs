//! Flags a function/event parameter that gets assigned a new value inside
//! its own body, since reusing the parameter's name for a different value
//! shadows what the caller passed in and can confuse a reader who expects
//! it to still reflect the original argument at any later point in the
//! function.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to tell a parameter's own name apart from an unrelated local or
//! property with the same name; a script that doesn't parse cleanly is
//! left unchecked rather than guessed at.

use papyrus_parser::ast::{Expr, FunctionDecl, Script, Stmt};

use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "parameter-reassignment";

/// Checks `source` for a function/event parameter reassigned somewhere in
/// its own body. Flagged as a `[warning]`.
///
/// A reassignment inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers,
/// is never flagged: it's generated boilerplate the user can't edit.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };
    let protected = fragment_code::protected_lines(source);

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        if function.params.is_empty() {
            continue;
        }
        for assign in collect_assigns(&function.body) {
            let Stmt::Assign { target, line, .. } = assign else {
                continue;
            };
            let Expr::Identifier(name) = target else {
                continue;
            };
            if protected.get(*line).copied().unwrap_or(false) {
                continue;
            }
            let Some(param) = function
                .params
                .iter()
                .find(|param| param.name.eq_ignore_ascii_case(name))
            else {
                continue;
            };

            diagnostics.push(Diagnostic {
                line: *line,
                column: 1,
                message: format!(
                    "[warning] Parameter '{}' is reassigned inside its function; consider using a local variable instead",
                    param.name
                ),
                rule: RULE,
            });
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
    fn flags_direct_parameter_reassignment() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test(Int total)\n    total = 1\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("'total'"));
    }

    #[test]
    fn flags_compound_parameter_reassignment() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test(Int total)\n    total += 1\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn matches_parameter_name_case_insensitively() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test(Int total)\n    TOTAL = 1\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_a_local_variable_reassignment() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int total)\n    Int other = 0\n    other = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_member_or_index_assignment_built_from_a_parameter() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    akRef.Foo = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_parameter_reassignment_inside_if_block() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int total)\n    If true\n        total = 1\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn flags_parameter_reassignment_inside_while_loop() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int total)\n    While total > 0\n        total -= 1\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Waiting\n    Function Test(Int total)\n        total = 1\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_flag_a_function_with_no_parameters() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Int total = 0\n    total = 1\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn returns_no_diagnostics_for_a_script_that_fails_to_parse() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test(Int total\n    total = 1\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }
}
