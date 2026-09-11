//! Flags a call reaching a script's `Global` function through an actual
//! object reference (a local variable, parameter, property, cast, or array
//! element — e.g. `akOtherActor.MyGlobalHelper()`) rather than Papyrus's
//! static/global call syntax (`ScriptName.MyGlobalHelper()`). Papyrus
//! allows this — a `Global` function ignores whatever reference it's
//! called through — but it can read as a mistake: a reader expecting an
//! instance method (or the author themselves, if the call was copy-pasted
//! from one) may not realize the reference is never actually used.
//!
//! The mirror image of [`crate::non_global_function_call`], which instead
//! flags a *non*-`Global` function reached through the static syntax.
//! `Self`/`Parent` are deliberately left unflagged: a script calling its
//! own `Global` function through `Self` for symmetry with its other
//! `Self.Whatever()` calls is a reasonable, common style rather than a
//! likely mistake. Whether a resolved function is declared `Global`
//! depends on the project's own scripts, which this crate has no
//! filesystem access to on its own; a caller that can resolve it (e.g. the
//! desktop app's `FunctionTable`) does so by implementing
//! [`ExternalSignatures::is_global_function`] and calling [`check_with`]
//! instead of [`check`].

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::argument_types::{ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "static-function-call-via-instance";

/// Checks `source` for calls reaching a `Global` function through an
/// object reference. Since this crate has no filesystem access on its own,
/// no such call can ever be confirmed this way; see [`check_with`] to
/// actually resolve function signatures.
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but resolves each call's target function through
/// `external`, flagging one that resolves and is declared `Global`.
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
                if !matches!(**object, Expr::Self_ | Expr::Parent) {
                    if let Some(object_type) = infer_type(object, env) {
                        if !object_type.is_array
                            && external.is_global_function(&object_type.name, property)
                                == Some(true)
                        {
                            diagnostics.push(called_via_instance(
                                *line,
                                *col,
                                &object_type.name,
                                property,
                            ));
                        }
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

fn called_via_instance(
    line: usize,
    col: usize,
    type_name: &str,
    function_name: &str,
) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!(
            "[warning] '{function_name}' is declared Global on '{type_name}', so it doesn't need an object reference — calling it as '{type_name}.{function_name}()' avoids the possibly-confusing implication that the object matters, but calling it through an instance still works"
        ),
        rule: RULE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_flag_anything_without_a_resolver() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(MyScript akRef)\n    akRef.SomeGlobal()\nEndFunction\n",
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

        fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
            if type_name.eq_ignore_ascii_case("MyScript")
                && function_name.eq_ignore_ascii_case("IsGlobal")
            {
                Some(true)
            } else if type_name.eq_ignore_ascii_case("MyScript")
                && function_name.eq_ignore_ascii_case("NotGlobal")
            {
                Some(false)
            } else {
                None
            }
        }
    }

    #[test]
    fn flags_a_global_function_called_through_a_local_variable() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(MyScript akRef)\n    akRef.IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("'IsGlobal' is declared Global on 'MyScript'"));
    }

    #[test]
    fn flags_a_global_function_called_through_a_property() {
        let diagnostics = check_with(
            "ScriptName Example\n\nMyScript Property Target Auto\n\nFunction Test()\n    Target.IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_global_function_called_through_an_array_element() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(MyScript[] refs, Int i)\n    refs[i].IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule, RULE);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].column, 21);
    }

    #[test]
    fn flags_a_global_function_called_through_a_cast() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Form value)\n    (value as MyScript).IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("calling it as 'MyScript.IsGlobal()'"));
    }

    #[test]
    fn does_not_flag_a_call_on_an_array_itself() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(MyScript[] refs)\n    refs.IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_instance_function_called_through_a_local_variable() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(MyScript akRef)\n    akRef.NotGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_call_through_self_or_parent() {
        let diagnostics = check_with(
            "ScriptName Example Extends MyScript\n\nFunction Test()\n    self.IsGlobal()\n    parent.IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_unresolved_object_or_function() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(MyScript akRef)\n    akRef.SomethingElse()\n    MyScript.IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn finds_a_call_in_nested_control_flow_and_state_functions() {
        let diagnostics = check_with(
            r#"
ScriptName Example

Function Test(MyScript akRef)
    If true
        akRef.IsGlobal()
    EndIf
EndFunction

State Active
    Function Run(MyScript akRef)
        akRef.IsGlobal()
    EndFunction
EndState
"#,
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn finds_calls_in_each_statement_and_expression_position() {
        let diagnostics = check_with(
            r#"ScriptName Example

Function Test(MyScript akRef)
    Bool initialized = akRef.IsGlobal()
    initialized = akRef.IsGlobal()
    If akRef.IsGlobal() && !akRef.IsGlobal()
    ElseIf akRef.IsGlobal()
    EndIf
    While akRef.IsGlobal()
        SomeCall(akRef.IsGlobal())
    EndWhile
    Return akRef.IsGlobal()
EndFunction
"#,
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 8);
        assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule == RULE));
    }

    #[test]
    fn resolves_type_and_function_names_case_insensitively() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(myscript akRef)\n    akRef.isglobal()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("'isglobal' is declared Global on 'myscript'"));
    }

    #[test]
    fn walks_an_indexed_receiver_used_as_a_plain_call_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(MyScript[] refs, Int i)\n    Debug.Trace(refs[i])\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_call_nested_inside_a_named_argument_and_walks_a_new_array_expression() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(MyScript akRef)\n    SomeCall(flag = akRef.IsGlobal())\n    Int[] arr = new Int[3]\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("IsGlobal"));
    }

    #[test]
    fn fake_external_lookup_always_returns_none() {
        assert!(FakeExternal.lookup("MyScript", "IsGlobal").is_none());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }
}
