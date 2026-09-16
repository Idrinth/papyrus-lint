//! Flags an `Import <ScriptName>` statement whose imported script's
//! `Global` functions are never called unqualified anywhere in this
//! script.
//!
//! Papyrus's `Import` statement lets a script call another script's
//! `Global` functions without qualifying them by that script's name (e.g.
//! `Import Utility` then `Wait(1.0)` instead of `Utility.Wait(1.0)`). Since
//! this crate has no filesystem access of its own, it can't tell whether
//! any given unqualified call actually resolves to one of an imported
//! script's `Global` functions; a caller that can resolve it (e.g. the
//! desktop app's `FunctionTable`) does so by implementing
//! [`ExternalSignatures::is_global_function`] and
//! [`ExternalSignatures::can_resolve_script`] and calling [`check_with`]
//! instead of [`check`]. An import whose script
//! [`ExternalSignatures::can_resolve_script`] can't confirm — including
//! every import when using [`NoExternalSignatures`], which never resolves
//! anything — is never flagged, so this can't mistake a lack of project
//! data for proof an import goes unused.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt};

use crate::argument_types::{ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-import";

/// Checks `source` for `Import` statements whose script goes unused. Since
/// this crate has no filesystem access on its own, no import can ever be
/// resolved this way, so nothing is ever flagged; see [`check_with`] to
/// actually resolve the imported scripts' `Global` functions.
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but resolves each unqualified call in `source` through
/// `external`, flagging an `Import` whose script never has one of its
/// `Global` functions called unqualified anywhere in `source`.
pub fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };
    if script.imports.is_empty() {
        return Vec::new();
    }

    let mut called_names = Vec::new();
    for function in all_functions(&script) {
        for stmt in &function.body {
            collect_stmt(stmt, &mut called_names);
        }
    }

    let mut diagnostics = Vec::new();
    for import in &script.imports {
        if !external.can_resolve_script(&import.name) {
            continue;
        }
        let is_used = called_names
            .iter()
            .any(|name| external.is_global_function(&import.name, name) == Some(true));
        if is_used {
            continue;
        }
        diagnostics.push(Diagnostic {
            line: import.line,
            column: 1,
            message: format!(
                "[warning] Import '{}' is never used: none of its Global functions are called unqualified anywhere in this script",
                import.name
            ),
            rule: RULE,
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

/// Collects the name of every bare, unqualified call found anywhere in
/// `stmt` (e.g. `Foo()`, but not `object.Foo()`) into `calls`.
fn collect_stmt(stmt: &Stmt, calls: &mut Vec<String>) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                collect_expr(value, calls);
            }
        }
        Stmt::Assign { target, value, .. } => {
            collect_expr(target, calls);
            collect_expr(value, calls);
        }
        Stmt::Expr { value, .. } => collect_expr(value, calls),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                collect_expr(value, calls);
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
                collect_expr(condition, calls);
                for stmt in body {
                    collect_stmt(stmt, calls);
                }
            }
            for stmt in else_body {
                collect_stmt(stmt, calls);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            collect_expr(condition, calls);
            for stmt in body {
                collect_stmt(stmt, calls);
            }
        }
    }
}

fn collect_expr(expr: &Expr, calls: &mut Vec<String>) {
    match expr {
        Expr::Call { callee, args, .. } => {
            if let Expr::Identifier(name) = &**callee {
                calls.push(name.clone());
            }
            collect_expr(callee, calls);
            for arg in args {
                collect_expr(arg, calls);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_expr(left, calls);
            collect_expr(right, calls);
        }
        Expr::Unary { operand, .. } => collect_expr(operand, calls),
        Expr::Member { object, .. } => collect_expr(object, calls),
        Expr::Index { object, index } => {
            collect_expr(object, calls);
            collect_expr(index, calls);
        }
        Expr::Cast { value, .. } => collect_expr(value, calls),
        Expr::NewArray { size, .. } => collect_expr(size, calls),
        Expr::NamedArg { value, .. } => collect_expr(value, calls),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::argument_types::ParamInfo;

    struct FakeExternal;

    impl ExternalSignatures for FakeExternal {
        fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
            None
        }

        fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
            if type_name.eq_ignore_ascii_case("B") && function_name.eq_ignore_ascii_case("BC") {
                Some(true)
            } else {
                None
            }
        }

        fn can_resolve_script(&mut self, type_name: &str) -> bool {
            type_name.eq_ignore_ascii_case("B") || type_name.eq_ignore_ascii_case("D")
        }
    }

    #[test]
    fn does_not_flag_anything_without_a_resolver() {
        let diagnostics =
            check("ScriptName A\n\nImport B\nImport D\n\nFunction C()\n    BC()\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_an_import_whose_global_function_is_never_called_unqualified() {
        let diagnostics = check_with(
            "ScriptName A\n\nImport B ; provided BC, so loaded\nImport D ; should be mentioned as warning - unused import\n\nFunction C()\n    BC()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule, RULE);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.contains("'D'"));
    }

    #[test]
    fn does_not_flag_an_import_used_through_a_call_in_a_nested_branch() {
        let diagnostics = check_with(
            "ScriptName A\n\nImport B\n\nFunction C(Bool flag)\n    If flag\n        BC()\n    EndIf\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_import_used_through_a_call_nested_in_an_argument() {
        let diagnostics = check_with(
            "ScriptName A\n\nImport B\n\nFunction C()\n    Debug.Trace(BC())\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_a_qualified_call_to_the_same_function_name() {
        let diagnostics = check_with(
            "ScriptName A\n\nImport B\n\nFunction C()\n    Other.BC()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'B'"));
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName A\n\nImport B\n\nFunction C(\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_nothing_when_the_script_has_no_imports() {
        let diagnostics = check_with(
            "ScriptName A\n\nFunction C()\nEndFunction\n",
            &mut FakeExternal,
        );
        assert!(diagnostics.is_empty());
    }
}
