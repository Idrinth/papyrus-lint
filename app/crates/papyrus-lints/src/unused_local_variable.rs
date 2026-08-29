//! Flags local variables that are declared but never read back: either
//! never referenced again at all ("unused"), or only ever assigned a new
//! value without that value ever being read ("write-only").
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to tell a variable's declaration and write sites apart from an actual
//! read of it; a script that doesn't parse cleanly is left unchecked
//! rather than guessed at. Papyrus has no block scoping — a local
//! declared inside an `If`/`While` body stays valid for the rest of its
//! function — so a variable's declaration and every reference to it are
//! matched across its whole enclosing function, by name,
//! case-insensitively (as Papyrus identifiers are). Function parameters
//! and script properties aren't locals and are never flagged here.

use std::collections::HashMap;

use papyrus_parser::ast::{AssignOp, Expr, FunctionDecl, Script, Stmt, VariableDecl};

use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-local-variable";

/// Checks `source` for local variable declarations whose value is never
/// read anywhere in their enclosing function. Flagged as a `[warning]`.
///
/// A declaration inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers, is
/// never flagged: it's CreationKit-generated boilerplate the user can't
/// edit or remove, so whether it's read from their own code isn't
/// something they can act on.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };
    let protected = fragment_code::protected_lines(source);

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        check_function(function, &protected, &mut diagnostics);
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

#[derive(Debug, Default, Clone, Copy)]
struct Usage {
    read: bool,
    written_after_declaration: bool,
}

fn check_function(function: &FunctionDecl, protected: &[bool], diagnostics: &mut Vec<Diagnostic>) {
    let decls = collect_var_decls(&function.body);
    if decls.is_empty() {
        return;
    }

    let mut usage: HashMap<String, Usage> = HashMap::new();
    for decl in &decls {
        usage.entry(decl.name.to_lowercase()).or_default();
    }
    walk_body(&function.body, &mut usage);

    for decl in decls {
        if protected.get(decl.line).copied().unwrap_or(false) {
            continue;
        }

        let info = usage
            .get(&decl.name.to_lowercase())
            .copied()
            .unwrap_or_default();
        if info.read {
            continue;
        }

        let message = if info.written_after_declaration {
            format!(
                "[warning] Local variable '{}' is assigned a value but never used",
                decl.name
            )
        } else {
            format!(
                "[warning] Local variable '{}' is declared but never used",
                decl.name
            )
        };

        diagnostics.push(Diagnostic {
            line: decl.line,
            column: 1,
            message,
            rule: RULE,
        });
    }
}

/// Finds every `VariableDecl` in `body`, including ones nested inside
/// `If`/`ElseIf`/`Else` branches and `While` bodies, since Papyrus locals
/// aren't block-scoped.
fn collect_var_decls(body: &[Stmt]) -> Vec<&VariableDecl> {
    let mut decls = Vec::new();
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => decls.push(decl),
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    decls.extend(collect_var_decls(&branch.body));
                }
                decls.extend(collect_var_decls(else_body));
            }
            Stmt::While { body, .. } => decls.extend(collect_var_decls(body)),
            _ => {}
        }
    }
    decls
}

fn walk_body(body: &[Stmt], usage: &mut HashMap<String, Usage>) {
    for stmt in body {
        walk_stmt(stmt, usage);
    }
}

fn walk_stmt(stmt: &Stmt, usage: &mut HashMap<String, Usage>) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr_as_read(value, usage);
            }
        }
        Stmt::Assign {
            target, op, value, ..
        } => {
            walk_expr_as_read(value, usage);
            match (target, op) {
                // A plain `x = ...` overwrites x without reading its
                // previous value, so it's a write rather than a use.
                (Expr::Identifier(name), AssignOp::Assign) => {
                    if let Some(entry) = usage.get_mut(&name.to_lowercase()) {
                        entry.written_after_declaration = true;
                    }
                }
                // A compound assignment (`x += 1`, ...) reads the current
                // value of x before writing the new one.
                (Expr::Identifier(name), _) => {
                    if let Some(entry) = usage.get_mut(&name.to_lowercase()) {
                        entry.read = true;
                        entry.written_after_declaration = true;
                    }
                }
                // A member/index assignment target (`foo.Bar = 1`,
                // `arr[0] = 1`) reads whatever local it's built from to
                // resolve the member/element being assigned into.
                _ => walk_expr_as_read(target, usage),
            }
        }
        Stmt::Expr { value, .. } => walk_expr_as_read(value, usage),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr_as_read(value, usage);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for branch in branches {
                walk_expr_as_read(&branch.condition, usage);
                walk_body(&branch.body, usage);
            }
            walk_body(else_body, usage);
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr_as_read(condition, usage);
            walk_body(body, usage);
        }
    }
}

fn walk_expr_as_read(expr: &Expr, usage: &mut HashMap<String, Usage>) {
    match expr {
        Expr::Identifier(name) => {
            if let Some(entry) = usage.get_mut(&name.to_lowercase()) {
                entry.read = true;
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr_as_read(left, usage);
            walk_expr_as_read(right, usage);
        }
        Expr::Unary { operand, .. } => walk_expr_as_read(operand, usage),
        Expr::Call { callee, args, .. } => {
            walk_expr_as_read(callee, usage);
            for arg in args {
                walk_expr_as_read(arg, usage);
            }
        }
        Expr::Member { object, .. } => walk_expr_as_read(object, usage),
        Expr::Index { object, index } => {
            walk_expr_as_read(object, usage);
            walk_expr_as_read(index, usage);
        }
        Expr::Cast { value, .. } => walk_expr_as_read(value, usage),
        Expr::NewArray { size, .. } => walk_expr_as_read(size, usage),
        Expr::Literal(_) | Expr::Self_ | Expr::Parent => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_local_variable_never_used_again() {
        let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("declared but never used"));
        assert!(diagnostics[0].message.contains("'i'"));
    }

    #[test]
    fn flags_write_only_local_variable() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int total = 0\n    total = 1\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0]
            .message
            .contains("assigned a value but never used"));
        assert!(diagnostics[0].message.contains("'total'"));
    }

    #[test]
    fn does_not_flag_variable_read_after_declaration() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int total = 0\n    Debug.Trace(total)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_variable_read_via_return() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Function Test()\n    Int total = 1\n    Return total\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_variable_used_via_compound_assignment() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int total = 0\n    total += 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_variable_only_indexed_or_accessed_through() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] arr = new Int[3]\n    arr[0] = 5\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn matches_variable_usage_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int total = 0\n    Debug.Trace(TOTAL)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_variable_declared_inside_if_block_never_used() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }

    #[test]
    fn does_not_flag_variable_declared_in_if_and_used_after_it() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    EndIf\n    Debug.Trace(i)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_function_parameters() {
        let diagnostics = check("ScriptName Example\n\nFunction Test(Int count)\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_script_properties() {
        let diagnostics = check("ScriptName Example\n\nInt Property MyValue = 1 Auto\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Int i = 1\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'i'"));
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }

    #[test]
    fn does_not_flag_a_generated_local_declared_in_a_fragment_wrapper() {
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment\n;NEXT FRAGMENT INDEX 0\nScriptname IDR__TIF__05000235 Extends TopicInfo Hidden\n\n;BEGIN FRAGMENT Fragment_0\nFunction Fragment_0(ObjectReference akSpeakerRef)\nActor akSpeaker = akSpeakerRef as Actor\n;BEGIN CODE\nPlayerRef.RemoveItem(Gold001, 5)\n;END CODE\nEndFunction\n;END FRAGMENT\n\n;END FRAGMENT CODE - Do not edit anything between this and the begin comment\nActor Property PlayerRef Auto\nMiscObject Property Gold001 Auto\n";

        assert!(check(source).is_empty());
    }

    #[test]
    fn still_flags_an_unused_local_declared_inside_the_code_block() {
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment\nScriptname Example Extends TopicInfo Hidden\nFunction Fragment_0(ObjectReference akSpeakerRef)\n;BEGIN CODE\nInt total = 1\n;END CODE\nEndFunction\n;END FRAGMENT CODE - Do not edit anything between this and the begin comment\n";

        let diagnostics = check(source);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);
    }
}
