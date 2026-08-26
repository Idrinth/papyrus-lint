//! Flags `If`/`ElseIf`/`While` conditions that aren't already boolean,
//! instead of relying on Papyrus's implicit conversion to `Bool`.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Stmt};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::Diagnostic;

/// Checks every `If`/`ElseIf`/`While` condition in `source` and flags the
/// ones that don't resolve to `Bool`.
///
/// A condition whose type can't be determined locally (a function call or a
/// member access on another script, for instance) is left unflagged rather
/// than risk a false positive.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut env = TypeEnv::for_script(&script);
    let mut diagnostics = Vec::new();

    for function in script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    ) {
        check_function(function, &mut env, &mut diagnostics);
    }

    diagnostics
}

fn check_function(function: &FunctionDecl, env: &mut TypeEnv, diagnostics: &mut Vec<Diagnostic>) {
    env.with_function_scope(function, |scoped| {
        check_body(&function.body, scoped, diagnostics);
    });
}

fn check_body(body: &[Stmt], env: &TypeEnv, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch {
                    condition,
                    body,
                    line,
                    col,
                } in branches
                {
                    check_condition(condition, *line, *col, env, diagnostics);
                    check_body(body, env, diagnostics);
                }
                check_body(else_body, env, diagnostics);
            }
            Stmt::While {
                condition,
                body,
                line,
                col,
            } => {
                check_condition(condition, *line, *col, env, diagnostics);
                check_body(body, env, diagnostics);
            }
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }
}

fn check_condition(
    condition: &Expr,
    line: usize,
    column: usize,
    env: &TypeEnv,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(type_name) = infer_type(condition, env) else {
        return;
    };

    if !type_name.is_array && type_name.name.eq_ignore_ascii_case("bool") {
        return;
    }

    let found = if type_name.is_array {
        format!("{}[]", type_name.name)
    } else {
        type_name.name
    };

    diagnostics.push(Diagnostic {
        line,
        column,
        message: format!("Condition must be a boolean value or expression, found '{found}'"),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_non_boolean_if_and_while_conditions() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int count)\n    If count\n    EndIf\n    While count\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 2);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (4, 5));
        assert!(diagnostics[0].message.contains("Int"));
        assert_eq!((diagnostics[1].line, diagnostics[1].column), (6, 5));
    }

    #[test]
    fn flags_each_elseif_branch_independently() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int a, Bool b)\n    If b\n    ElseIf a\n    Else\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn allows_boolean_literals_comparisons_and_logical_expressions() {
        let diagnostics = check(
            r#"
ScriptName Example

Function Test(Int a, Bool flag)
    If true
    EndIf
    If flag
    EndIf
    If a > 0
    EndIf
    If flag && a > 0
    EndIf
    If !flag
    EndIf
    While a > 0
    EndWhile
EndFunction
"#,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_conditions_that_cannot_be_resolved_locally() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If GetValue()\n    EndIf\n    If Self.SomeProperty\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_nested_and_state_function_bodies() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Int a)\n        If a > 0\n            If a\n            EndIf\n        EndIf\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }
}
