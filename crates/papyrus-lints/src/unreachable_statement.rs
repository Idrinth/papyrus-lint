//! Flags statements that appear after a `Return` within the same block,
//! since control flow can never reach them.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! the block structure of the function body; a script that doesn't parse
//! cleanly is left unchecked rather than guessed at.

use papyrus_parser::ast::{FunctionDecl, Script, Stmt};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unreachable-statement";

/// Checks `source` for statements that follow a `Return` in the same
/// block (a function/event body, an `If`/`ElseIf`/`Else` branch, or a
/// `While` body). Flagged as a `[warning]`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        check_body(&function.body, &mut diagnostics);
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

fn check_body(body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    let mut returned = false;
    for stmt in body {
        if returned {
            diagnostics.push(Diagnostic {
                line: stmt_line(stmt),
                column: 1,
                message: "[warning] Unreachable statement: this can never execute because the \
                          block already returned above it"
                    .to_string(),
                rule: RULE,
            });
        }
        match stmt {
            Stmt::Return { .. } => returned = true,
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    check_body(&branch.body, diagnostics);
                }
                check_body(else_body, diagnostics);
            }
            Stmt::While { body, .. } => check_body(body, diagnostics),
            _ => {}
        }
    }
}

fn stmt_line(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::VarDecl(decl) => decl.line,
        Stmt::Assign { line, .. } => *line,
        Stmt::Expr { line, .. } => *line,
        Stmt::Return { line, .. } => *line,
        Stmt::If { line, .. } => *line,
        Stmt::While { line, .. } => *line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_statement_after_return_in_function_body() {
        let source =
            "ScriptName Example\n\nFunction Test()\n    Return\n    Int i = 1\nEndFunction\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
    }

    #[test]
    fn flags_every_statement_after_the_first_return() {
        let source = "ScriptName Example\n\nFunction Test()\n    Return\n    Int i = 1\n    Int j = 2\nEndFunction\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].line, 5);
        assert_eq!(diagnostics[1].line, 6);
    }

    #[test]
    fn does_not_flag_a_trailing_return() {
        let source =
            "ScriptName Example\n\nFunction Test()\n    Int i = 1\n    Return\nEndFunction\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn does_not_flag_function_with_no_return() {
        let source = "ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn flags_statement_after_return_inside_if_branch() {
        let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Return\n        Int i = 1\n    EndIf\nEndFunction\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn flags_statement_after_return_inside_else_branch() {
        let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    Else\n        Return\n        Int j = 2\n    EndIf\nEndFunction\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 8);
    }

    #[test]
    fn flags_statement_after_return_inside_while_body() {
        let source = "ScriptName Example\n\nFunction Test()\n    While true\n        Return\n        Int i = 1\n    EndWhile\nEndFunction\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn a_return_inside_an_if_does_not_flag_statements_after_the_if_itself() {
        let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Return\n    EndIf\n    Int i = 1\nEndFunction\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let source = "ScriptName Example\n\nState Active\n    Function Test()\n        Return\n        Int i = 1\n    EndFunction\nEndState\n";

        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }
}
