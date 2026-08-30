//! Flags a call through Papyrus's static/global call syntax
//! (`ScriptName.Function(...)`, e.g. `Utility.Wait(1.0)` or
//! `MyMissingScript.DoThing()`) whose target script can't be located,
//! since Papyrus resolves that name against a script file at compile time
//! and a call through a script that doesn't exist can never compile.
//!
//! Only a call whose object is a bare identifier not already known as a
//! local variable, parameter, or property (i.e. definitely not an
//! instance the script already has a handle to) is treated as a script
//! reference at all — anything resolvable locally is left to the
//! "Argument type check"/"Return type check" lints instead. Whether such a
//! name can be located depends on the project's own scripts and the
//! engine's native singleton scripts (`Game`, `Utility`, `Debug`, ...),
//! neither of which this crate has access to on its own; a caller that can
//! resolve them (e.g. the desktop app's `FunctionTable`) does so by
//! implementing [`ExternalSignatures::script_exists`] and calling
//! [`check_with`] instead of [`check`].

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt};
use papyrus_parser::types::TypeEnv;

use crate::argument_types::{ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unresolved-script";

/// Checks `source` for calls through a script name that can't be
/// resolved. Since this crate has no filesystem access on its own, no
/// script can ever be confirmed missing this way; see [`check_with`] to
/// actually resolve script names.
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but resolves each call's target script through
/// `external`, flagging one that can't be located.
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
            if let Expr::Member { object, .. } = &**callee {
                if let Expr::Identifier(name) = &**object {
                    if env.lookup(name).is_none() && !external.script_exists(name) {
                        diagnostics.push(missing(*line, *col, name));
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

fn missing(line: usize, col: usize, name: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!("[warning] Script '{name}' could not be located"),
        rule: RULE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_flag_anything_without_a_resolver() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    MyMissingScript.StaticCall()\nEndFunction\n",
        );

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

        fn script_exists(&mut self, type_name: &str) -> bool {
            type_name.eq_ignore_ascii_case("Utility")
        }
    }

    #[test]
    fn flags_a_call_through_a_script_that_cannot_be_located() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test()\n    Int a = MyMissingScript.StaticCall()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("Script 'MyMissingScript' could not be located"));
    }

    #[test]
    fn does_not_flag_a_known_native_singleton_script() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test()\n    Utility.Wait(1.0)\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_call_through_a_local_variable_property_or_self() {
        let diagnostics = check_with(
            r#"
ScriptName Example

Actor Property PlayerRef Auto

Function Test(ObjectReference akRef)
    akRef.SendAnimationEvent("Wave")
    PlayerRef.GetName()
    self.Test(None)
EndFunction
"#,
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }
}
