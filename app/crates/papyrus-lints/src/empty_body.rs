//! Flags `While` loops, `If`/`ElseIf` branches, and `Else` blocks whose body
//! has no real effect, since that's almost always a forgotten piece of
//! logic rather than something intentional.
//!
//! A `While` loop counts as empty both when it has no statements at all and
//! when every statement in it only nudges a variable by a constant amount
//! (`i += 1`, `i -= 1`, or the equivalent `i = i + 1`/`i = i - 1`) — the
//! common "loop until a counter reaches some value" idiom, but with nothing
//! else in the loop that would give it a purpose. A step built from
//! anything but a literal (a call, another variable, ...) has a side effect
//! of its own and isn't considered trivial.
//!
//! `If`/`ElseIf` branches are checked straight from the parsed AST, since
//! an empty branch body is unambiguous there. An `Else` block is too: the
//! AST's `Stmt::If` records the line/column the `Else` keyword itself
//! started on (`None` when there was no `Else` clause at all), which is
//! what lets "no `Else` clause" be told apart from "an empty `Else`
//! clause" — both leave `else_body` empty — without re-lexing the source.
//!
//! A script that doesn't parse cleanly is simply left unchecked by the
//! `While`/`If`/`ElseIf`/`Else` checks above, all of which run from the
//! AST; as a fallback for that case only, the `Else` check also scans the
//! token stream directly for an `Else` keyword immediately followed (only
//! whitespace/newlines between them) by `EndIf`, so it still runs on a
//! script that doesn't parse.

use papyrus_parser::ast::{AssignOp, BinaryOp, Expr, FunctionDecl, Literal, Script, Stmt};
use papyrus_parser::lexer::Lexer;
use papyrus_parser::token::{Keyword, TokenKind};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "empty-body";

/// Checks `source` for `While` loops with no real effect and empty
/// `If`/`ElseIf`/`Else` bodies. Flagged as a `[warning]`, since this is
/// almost always an oversight rather than something intentional.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        // The AST can't tell an empty `Else` apart from no `Else` clause at
        // all without parsing, so fall back to scanning tokens directly for
        // this one case on a script that doesn't parse cleanly.
        return empty_else_diagnostics(source);
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
    for stmt in body {
        match stmt {
            Stmt::If {
                branches,
                else_body,
                else_line,
                else_col,
                ..
            } => {
                for branch in branches {
                    if branch.body.is_empty() {
                        diagnostics.push(Diagnostic {
                            line: branch.line,
                            column: branch.col,
                            message: "[warning] Empty If/ElseIf body; this looks like an \
                                      oversight rather than something intentional"
                                .to_string(),
                            rule: RULE,
                        });
                    }
                    check_body(&branch.body, diagnostics);
                }
                if let (Some(line), Some(column)) = (else_line, else_col) {
                    if else_body.is_empty() {
                        diagnostics.push(Diagnostic {
                            line: *line,
                            column: *column,
                            message: "[warning] Empty Else body; this looks like an oversight \
                                      rather than something intentional"
                                .to_string(),
                            rule: RULE,
                        });
                    }
                }
                check_body(else_body, diagnostics);
            }
            Stmt::While {
                body, line, col, ..
            } => {
                if body.is_empty() {
                    diagnostics.push(Diagnostic {
                        line: *line,
                        column: *col,
                        message: "[warning] Loop body is empty; this looks like an oversight \
                                  rather than something intentional"
                            .to_string(),
                        rule: RULE,
                    });
                } else if is_trivial_loop_body(body) {
                    diagnostics.push(Diagnostic {
                        line: *line,
                        column: *col,
                        message: "[warning] Loop only increments or decrements a variable, \
                                  with no other effect; this looks like an oversight rather \
                                  than something intentional"
                            .to_string(),
                        rule: RULE,
                    });
                }
                check_body(body, diagnostics);
            }
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }
}

/// Whether every statement in a (non-empty) loop body is nothing more than
/// a constant step applied to some variable.
fn is_trivial_loop_body(body: &[Stmt]) -> bool {
    body.iter().all(|stmt| match stmt {
        Stmt::Assign {
            target, op, value, ..
        } => is_constant_step(target, *op, value),
        _ => false,
    })
}

/// Whether `target <op> value` amounts to nudging `target` by a constant,
/// e.g. `i += 1`, `i -= 1`, `i = i + 1`, or `i = i - 1`. Anything the step
/// depends on beyond a literal (a call, another variable, ...) has a side
/// effect of its own and isn't considered trivial.
fn is_constant_step(target: &Expr, op: AssignOp, value: &Expr) -> bool {
    let Expr::Identifier(name) = target else {
        return false;
    };

    match op {
        AssignOp::AddAssign | AssignOp::SubAssign => is_numeric_literal(value),
        AssignOp::Assign => match value {
            Expr::Binary {
                left,
                op: BinaryOp::Add,
                right,
            } => {
                (is_identifier(left, name) && is_numeric_literal(right))
                    || (is_numeric_literal(left) && is_identifier(right, name))
            }
            Expr::Binary {
                left,
                op: BinaryOp::Sub,
                right,
            } => is_identifier(left, name) && is_numeric_literal(right),
            _ => false,
        },
        AssignOp::MulAssign | AssignOp::DivAssign | AssignOp::ModAssign => false,
    }
}

fn is_identifier(expr: &Expr, name: &str) -> bool {
    matches!(expr, Expr::Identifier(other) if other.eq_ignore_ascii_case(name))
}

fn is_numeric_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Literal(Literal::Int { .. }) | Expr::Literal(Literal::Float(_))
    )
}

/// Fallback for a script that doesn't parse cleanly (see [`check`]): scans
/// `source`'s lexer tokens directly for an `Else` keyword immediately
/// followed (modulo newlines) by `EndIf`.
fn empty_else_diagnostics(source: &str) -> Vec<Diagnostic> {
    let Ok(tokens) = Lexer::new(source).tokenize() else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Keyword(Keyword::Else) {
            continue;
        }
        let next = tokens[index + 1..]
            .iter()
            .find(|token| token.kind != TokenKind::Newline);
        if matches!(next, Some(token) if token.kind == TokenKind::Keyword(Keyword::EndIf)) {
            diagnostics.push(Diagnostic {
                line: token.line,
                column: token.col,
                message: "[warning] Empty Else body; this looks like an oversight rather than \
                          something intentional"
                    .to_string(),
                rule: RULE,
            });
        }
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_completely_empty_while_loop() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    While true\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.contains("Loop body is empty"));
    }

    #[test]
    fn flags_while_loop_that_only_increments_its_counter() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 0\n    While i < 10\n        i += 1\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
        assert!(diagnostics[0].message.contains("increments or decrements"));
    }

    #[test]
    fn flags_while_loop_that_only_decrements_its_counter() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 10\n    While i > 0\n        i -= 1\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("increments or decrements"));
    }

    #[test]
    fn flags_while_loop_using_plain_assignment_increment() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 0\n    While i < 10\n        i = i + 1\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);

        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 10\n    While i > 0\n        i = i - 1\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_a_loop_with_actual_content() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 0\n    While i < 10\n        i += 1\n        Debug.Trace(\"tick\")\n    EndWhile\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_step_built_from_a_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 0\n    While i < 10\n        i += GetStep()\n    EndWhile\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_multiplicative_step() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i = 1\n    While i < 100\n        i *= 2\n    EndWhile\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_empty_if_body() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.contains("Empty If/ElseIf body"));
    }

    #[test]
    fn flags_empty_elseif_body() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool a, Bool b)\n    If a\n        Debug.Trace(\"a\")\n    ElseIf b\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn flags_empty_else_body() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n        Debug.Trace(\"a\")\n    Else\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
        assert!(diagnostics[0].message.contains("Empty Else body"));
    }

    #[test]
    fn does_not_flag_an_if_with_no_else_clause_at_all() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n        Debug.Trace(\"a\")\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_non_empty_else_body() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n        Debug.Trace(\"a\")\n    Else\n        Debug.Trace(\"b\")\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_nested_and_state_bodies() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Bool flag)\n        If flag\n            While true\n            EndWhile\n        EndIf\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }

    #[test]
    fn empty_else_check_still_runs_on_unparseable_source() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test(\n    If flag\n    Else\n    EndIf\n");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Empty Else body"));
    }
}
