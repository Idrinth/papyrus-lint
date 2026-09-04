//! Flags a call through Papyrus's static/global call syntax
//! (`ScriptName.Function(...)`, e.g. `MyScript.DoThing()`) whose target
//! function isn't declared `Global` on that script — Papyrus only allows
//! the static syntax to reach a script's `Global` functions; calling an
//! ordinary instance function that way fails to compile, since it needs an
//! actual object reference (or `Self`) to run against.
//!
//! Like [`crate::unresolved_script`], only a call whose object is a bare
//! identifier not already known as a local variable, parameter, or
//! property is treated as a script reference at all — anything resolvable
//! locally is a normal instance call, left to the "Argument type check"
//! lint instead. Whether a resolved function is declared `Global` depends
//! on the project's own scripts, which this crate has no filesystem access
//! to on its own; a caller that can resolve it (e.g. the desktop app's
//! `FunctionTable`) does so by implementing
//! [`ExternalSignatures::is_global_function`] and calling [`check_with`]
//! instead of [`check`].

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt};
use papyrus_parser::types::TypeEnv;

use crate::argument_types::{ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "non-global-function-call";

/// Checks `source` for calls through a script name whose target function
/// isn't declared `Global`. Since this crate has no filesystem access on
/// its own, no such call can ever be confirmed this way; see
/// [`check_with`] to actually resolve function signatures.
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but resolves each call's target function through
/// `external`, flagging one that resolves but isn't declared `Global`.
pub fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut env = TypeEnv::for_script(&script);
    let mut diagnostics = Vec::new();

    for function in all_functions(&script) {
        env.with_function_scope(function, |env| {
            for stmt in &function.body {
                walk_stmt(stmt, env, external, &mut diagnostics);
            }
        });
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
    env: &TypeEnv,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr(value, env, external, diagnostics);
            }
        }
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, env, external, diagnostics);
            walk_expr(value, env, external, diagnostics);
        }
        Stmt::Expr { value, .. } => walk_expr(value, env, external, diagnostics),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr(value, env, external, diagnostics);
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
                walk_expr(condition, env, external, diagnostics);
                for stmt in body {
                    walk_stmt(stmt, env, external, diagnostics);
                }
            }
            for stmt in else_body {
                walk_stmt(stmt, env, external, diagnostics);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, env, external, diagnostics);
            for stmt in body {
                walk_stmt(stmt, env, external, diagnostics);
            }
        }
    }
}

fn walk_expr<E: ExternalSignatures>(
    expr: &Expr,
    env: &TypeEnv,
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
            if let Expr::Member { object, property } = &**callee {
                if let Expr::Identifier(name) = &**object {
                    if env.lookup(name).is_none()
                        && external.is_global_function(name, property) == Some(false)
                    {
                        diagnostics.push(not_global(*line, *col, name, property));
                    }
                }
            }
            walk_expr(callee, env, external, diagnostics);
            for arg in args {
                walk_expr(arg, env, external, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, env, external, diagnostics);
            walk_expr(right, env, external, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, env, external, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, env, external, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, env, external, diagnostics);
            walk_expr(index, env, external, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, env, external, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, env, external, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, env, external, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

fn not_global(line: usize, col: usize, type_name: &str, function_name: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!(
            "[error] '{function_name}' is not declared Global on '{type_name}', so it can't be called as '{type_name}.{function_name}()' without an instance"
        ),
        rule: RULE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_flag_anything_without_a_resolver() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    MyScript.NotGlobal()\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    struct FakeExternal;

    impl ExternalSignatures for FakeExternal {
        fn lookup(
            &mut self,
            _type_name: &str,
            _function_name: &str,
        ) -> Option<Vec<crate::argument_types::ParamInfo>> {
            None
        }

        fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
            if type_name.eq_ignore_ascii_case("MyScriptOne")
                && function_name.eq_ignore_ascii_case("IMNotStatic")
            {
                Some(false)
            } else if type_name.eq_ignore_ascii_case("Utility")
                && function_name.eq_ignore_ascii_case("Wait")
            {
                Some(true)
            } else {
                None
            }
        }
    }

    #[test]
    fn flags_a_call_through_a_script_name_to_a_non_global_function() {
        let diagnostics = check_with(
            "ScriptName MyScriptTwo\n\nFunction Mine()\n    Return MyScriptOne.IMNotStatic()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("'IMNotStatic' is not declared Global on 'MyScriptOne'"));
    }

    #[test]
    fn does_not_flag_a_call_to_a_known_global_function() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(1.0)\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_unresolved_script_or_function() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test()\n    MyMissingScript.DoThing()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_call_through_a_local_variable_property_or_self() {
        let diagnostics = check_with(
            r#"
ScriptName Example

MyScriptOne Property Target Auto

Function Test(MyScriptOne akRef)
    akRef.IMNotStatic()
    Target.IMNotStatic()
    self.Test(None)
EndFunction
"#,
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn finds_a_non_global_call_in_nested_control_flow_and_state_functions() {
        let diagnostics = check_with(
            r#"
ScriptName Example

Function Test()
    If true
        MyScriptOne.IMNotStatic()
    EndIf
EndFunction

State Active
    Function Run()
        MyScriptOne.IMNotStatic()
    EndFunction
EndState
"#,
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }
}
