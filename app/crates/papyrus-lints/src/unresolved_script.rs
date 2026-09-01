//! Flags an unresolved parent script, declared type, or call through
//! Papyrus's static/global call syntax
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
//! name, an `Extends` parent, or a type annotation can be located depends
//! on the project's own scripts and built-in native type data,
//! neither of which this crate has access to on its own; a caller that can
//! resolve them (e.g. the desktop app's `FunctionTable`) does so by
//! implementing [`ExternalSignatures::script_exists`] and
//! [`ExternalSignatures::type_exists`] and calling
//! [`check_with`] instead of [`check`].

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt, TypeName};
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

    if let Some(parent) = &script.extends {
        if !external.type_exists(parent) {
            diagnostics.push(missing_type(
                header_line(source),
                1,
                parent,
                "Parent script",
            ));
        }
    }

    for property in &script.properties {
        check_type(
            &property.type_name,
            property.line,
            external,
            &mut diagnostics,
        );
        if let Some(value) = &property.value {
            walk_expr(value, property.line, &env, external, &mut diagnostics);
        }
    }
    for variable in &script.variables {
        check_type(
            &variable.type_name,
            variable.line,
            external,
            &mut diagnostics,
        );
        if let Some(value) = &variable.value {
            walk_expr(value, variable.line, &env, external, &mut diagnostics);
        }
    }

    for function in all_functions(&script) {
        if let Some(return_type) = &function.return_type {
            check_type(return_type, function.line, external, &mut diagnostics);
        }
        for param in &function.params {
            check_type(&param.type_name, function.line, external, &mut diagnostics);
        }
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
            check_type(&decl.type_name, decl.line, external, diagnostics);
            if let Some(value) = &decl.value {
                walk_expr(value, decl.line, env, external, diagnostics);
            }
        }
        Stmt::Assign {
            target,
            value,
            line,
            ..
        } => {
            walk_expr(target, *line, env, external, diagnostics);
            walk_expr(value, *line, env, external, diagnostics);
        }
        Stmt::Expr { value, line } => walk_expr(value, *line, env, external, diagnostics),
        Stmt::Return { value, line } => {
            if let Some(value) = value {
                walk_expr(value, *line, env, external, diagnostics);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for IfBranch {
                condition,
                body,
                line,
                ..
            } in branches
            {
                walk_expr(condition, *line, env, external, diagnostics);
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
            walk_expr(condition, stmt_line(stmt), env, external, diagnostics);
            for stmt in body {
                walk_stmt(stmt, env, external, diagnostics);
            }
        }
    }
}

fn walk_expr<E: ExternalSignatures>(
    expr: &Expr,
    line: usize,
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
            walk_expr(callee, *line, env, external, diagnostics);
            for arg in args {
                walk_expr(arg, *line, env, external, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, line, env, external, diagnostics);
            walk_expr(right, line, env, external, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, line, env, external, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, line, env, external, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, line, env, external, diagnostics);
            walk_expr(index, line, env, external, diagnostics);
        }
        Expr::Cast { value, type_name } => {
            check_type_name(type_name, line, external, diagnostics);
            walk_expr(value, line, env, external, diagnostics);
        }
        Expr::NewArray { type_name, size } => {
            check_type(type_name, line, external, diagnostics);
            walk_expr(size, line, env, external, diagnostics);
        }
        Expr::NamedArg { value, .. } => walk_expr(value, line, env, external, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

fn header_line(source: &str) -> usize {
    source
        .lines()
        .position(|line| line.to_ascii_lowercase().contains("scriptname"))
        .map_or(1, |line| line + 1)
}

fn stmt_line(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::VarDecl(decl) => decl.line,
        Stmt::Assign { line, .. }
        | Stmt::Expr { line, .. }
        | Stmt::Return { line, .. }
        | Stmt::If { line, .. }
        | Stmt::While { line, .. } => *line,
    }
}

fn check_type<E: ExternalSignatures>(
    type_name: &TypeName,
    line: usize,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_type_name(&type_name.name, line, external, diagnostics);
}

fn check_type_name<E: ExternalSignatures>(
    type_name: &str,
    line: usize,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !external.type_exists(type_name) {
        diagnostics.push(missing_type(line, 1, type_name, "Type"));
    }
}

fn missing_type(line: usize, col: usize, name: &str, kind: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!("[warning] {kind} '{name}' could not be located"),
        rule: RULE,
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

        fn type_exists(&mut self, type_name: &str) -> bool {
            matches!(
                type_name.to_ascii_lowercase().as_str(),
                "int" | "bool" | "known" | "actor" | "objectreference"
            )
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
    fn flags_unresolved_parent_and_declared_types_on_their_lines() {
        let diagnostics = check_with(
            "\nScriptName Example Extends MissingParent\n\nMissingProperty Property Value Auto\n\nMissingReturn Function Test(Known ok, MissingParam bad)\n    MissingLocal local\n    local = local as MissingCast\n    MissingArray[] values = new MissingElement[1]\nEndFunction\n",
            &mut FakeExternal,
        );

        let findings: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.line, diagnostic.message.as_str()))
            .collect();
        assert!(findings.contains(&(
            2,
            "[warning] Parent script 'MissingParent' could not be located"
        )));
        assert!(findings.contains(&(4, "[warning] Type 'MissingProperty' could not be located")));
        assert!(findings.contains(&(6, "[warning] Type 'MissingReturn' could not be located")));
        assert!(findings.contains(&(6, "[warning] Type 'MissingParam' could not be located")));
        assert!(findings.contains(&(7, "[warning] Type 'MissingLocal' could not be located")));
        assert!(findings.contains(&(8, "[warning] Type 'MissingCast' could not be located")));
        assert!(findings.contains(&(9, "[warning] Type 'MissingArray' could not be located")));
        assert!(findings.contains(&(9, "[warning] Type 'MissingElement' could not be located")));
        assert!(findings
            .iter()
            .all(|(_, message)| !message.contains("Known")));
    }

    #[test]
    fn finds_missing_scripts_in_nested_control_flow_and_expressions() {
        let diagnostics = check_with(
            r#"
ScriptName Example

Function Test()
    Int[] values = new Int[MissingSize.Get()]
    values[MissingIndex.Get()] = MissingValue.Get()
    If MissingCondition.Get() && !MissingGuard.Get()
        MissingBody.Run(MissingArgument.Get())
    ElseIf MissingAlternative.Get()
        Return
    Else
        While MissingLoop.Get()
            values[0] = (MissingCast.Get() as Int)
        EndWhile
    EndIf
EndFunction
"#,
            &mut FakeExternal,
        );

        let mut missing_scripts: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| {
                diagnostic
                    .message
                    .strip_prefix("[warning] Script '")
                    .and_then(|message| message.strip_suffix("' could not be located"))
                    .unwrap()
            })
            .collect();
        missing_scripts.sort_unstable();

        assert_eq!(
            missing_scripts,
            [
                "MissingAlternative",
                "MissingArgument",
                "MissingBody",
                "MissingCast",
                "MissingCondition",
                "MissingGuard",
                "MissingIndex",
                "MissingLoop",
                "MissingSize",
                "MissingValue",
            ]
        );
    }

    #[test]
    fn finds_missing_scripts_in_return_values_and_state_functions() {
        let diagnostics = check_with(
            r#"
ScriptName Example

Int Function GetValue()
    Return MissingReturn.Get()
EndFunction

State Active
    Function Run()
        MissingState.Run()
    EndFunction
EndState
"#,
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0].message.contains("MissingReturn"));
        assert!(diagnostics[1].message.contains("MissingState"));
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
