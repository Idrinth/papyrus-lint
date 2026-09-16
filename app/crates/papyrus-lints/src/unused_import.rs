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
//!
//! [`repair_with`] removes exactly the `Import` lines [`check_with`]
//! flags, deleting each whole line (including its line ending) rather than
//! leaving a blank one behind; [`repair`] is its [`check`] counterpart,
//! never removing anything since [`NoExternalSignatures`] never resolves a
//! script either.

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

/// Removes every `Import` line [`check`] flags — which, since it never
/// resolves anything (see [`NoExternalSignatures`]), is none at all. See
/// [`repair_with`] to actually remove imports resolved unused through a
/// project's own external signatures.
pub fn repair(source: &str) -> String {
    repair_with(source, &mut NoExternalSignatures)
}

/// Like [`repair`], but removes every `Import` line [`check_with`] flags
/// as unused through `external`, deleting each whole line (including its
/// line ending) rather than leaving a blank line in its place. A script
/// that doesn't parse cleanly, or that [`check_with`] finds nothing to
/// flag in, is returned unchanged.
pub fn repair_with<E: ExternalSignatures>(source: &str, external: &mut E) -> String {
    let lines_to_remove: std::collections::HashSet<usize> = check_with(source, external)
        .into_iter()
        .map(|diagnostic| diagnostic.line)
        .collect();
    if lines_to_remove.is_empty() {
        return source.to_string();
    }

    let mut result = String::with_capacity(source.len());
    let mut rest = source;
    let mut line_number = 1usize;
    while !rest.is_empty() {
        let (line_and_ending, remainder) = match rest.find('\n') {
            Some(index) => (&rest[..=index], &rest[index + 1..]),
            None => (rest, ""),
        };
        if !lines_to_remove.contains(&line_number) {
            result.push_str(line_and_ending);
        }
        rest = remainder;
        line_number += 1;
    }
    result
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

    #[test]
    fn repair_removes_only_the_unused_imports_line() {
        let source = "ScriptName A\n\nImport B ; provided BC, so loaded\nImport D ; unused\n\nFunction C()\n    BC()\nEndFunction\n";

        let repaired = repair_with(source, &mut FakeExternal);

        assert_eq!(
            repaired,
            "ScriptName A\n\nImport B ; provided BC, so loaded\n\nFunction C()\n    BC()\nEndFunction\n"
        );
        assert!(check_with(&repaired, &mut FakeExternal).is_empty());
    }

    #[test]
    fn repair_removes_multiple_unused_import_lines() {
        let source = "ScriptName A\n\nImport B\nImport D\n\nFunction C()\nEndFunction\n";

        let repaired = repair_with(source, &mut FakeExternal);

        assert_eq!(repaired, "ScriptName A\n\n\nFunction C()\nEndFunction\n");
    }

    #[test]
    fn repair_leaves_a_used_imports_only_script_unchanged() {
        let source = "ScriptName A\n\nImport B\n\nFunction C()\n    BC()\nEndFunction\n";

        assert_eq!(repair_with(source, &mut FakeExternal), source);
    }

    #[test]
    fn repair_without_a_resolver_never_removes_anything() {
        let source = "ScriptName A\n\nImport B\nImport D\n\nFunction C()\n    BC()\nEndFunction\n";

        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_preserves_crlf_line_endings() {
        let source = "ScriptName A\r\n\r\nImport D\r\n\r\nFunction C()\r\nEndFunction\r\n";

        let repaired = repair_with(source, &mut FakeExternal);

        assert_eq!(
            repaired,
            "ScriptName A\r\n\r\n\r\nFunction C()\r\nEndFunction\r\n"
        );
    }

    #[test]
    fn repair_does_not_crash_on_unparseable_source() {
        let source = "ScriptName A\n\nImport B\n\nFunction C(\nEndFunction\n";

        assert_eq!(repair_with(source, &mut FakeExternal), source);
    }
}
