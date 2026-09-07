//! Flags an assignment that writes to a script-level property declared
//! `AutoReadOnly` (e.g. `Float Property a = 0.1 AutoReadOnly`), since
//! Papyrus rejects such an assignment at compile time — an `AutoReadOnly`
//! property can only ever be set to its declared initial value.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to tell an `AutoReadOnly` property's own name apart from an unrelated
//! local variable or parameter that happens to share it; a script that
//! doesn't parse cleanly is left unchecked rather than guessed at.

use std::collections::HashSet;

use papyrus_parser::ast::{Expr, FunctionDecl, Script, Stmt, VariableDecl};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "readonly-property-write";

/// Checks `source` for an assignment (`=`, `+=`, `-=`, ...) targeting a
/// script-level property declared `AutoReadOnly`, either by its bare name
/// or as `Self.PropertyName`. A bare name shadowed by a same-named local
/// variable or parameter in the enclosing function refers to that local/
/// parameter instead, and is never flagged. Flagged as an `[error]`, since
/// Papyrus rejects the assignment at compile time.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let readonly_properties: HashSet<String> = script
        .properties
        .iter()
        .filter(|property| property.is_auto_read_only)
        .map(|property| property.name.to_ascii_lowercase())
        .collect();
    if readonly_properties.is_empty() {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        let shadowed: HashSet<String> = function
            .params
            .iter()
            .map(|param| param.name.to_ascii_lowercase())
            .chain(
                collect_var_decls(&function.body)
                    .into_iter()
                    .map(|decl| decl.name.to_ascii_lowercase()),
            )
            .collect();

        for assign in collect_assigns(&function.body) {
            let Stmt::Assign { target, line, .. } = assign else {
                continue;
            };
            let Some(target) = target_property(target) else {
                continue;
            };
            let name_lower = target.name.to_ascii_lowercase();
            if !readonly_properties.contains(&name_lower) {
                continue;
            }
            if !target.is_self_qualified && shadowed.contains(&name_lower) {
                continue;
            }

            diagnostics.push(Diagnostic {
                line: *line,
                column: 1,
                message: format!(
                    "[error] '{}' is declared AutoReadOnly and cannot be assigned a new value",
                    target.name
                ),
                rule: RULE,
            });
        }
    }
    diagnostics
}

/// A property an assignment's target expression resolves to, either by its
/// bare name or as `Self.PropertyName`.
struct PropertyTarget<'a> {
    name: &'a str,
    is_self_qualified: bool,
}

/// If `target` is a bare identifier or a `Self.PropertyName` member access,
/// returns the referenced name. Anything else (a member access on
/// something other than `Self`, an index, ...) returns `None` rather than
/// being guessed at.
fn target_property(target: &Expr) -> Option<PropertyTarget<'_>> {
    match target {
        Expr::Identifier(name) => Some(PropertyTarget {
            name,
            is_self_qualified: false,
        }),
        Expr::Member { object, property } if matches!(object.as_ref(), Expr::Self_) => {
            Some(PropertyTarget {
                name: property,
                is_self_qualified: true,
            })
        }
        _ => None,
    }
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

/// Finds every `Assign` statement in `body`, including ones nested inside
/// `If`/`ElseIf`/`Else` branches and `While` bodies.
fn collect_assigns(body: &[Stmt]) -> Vec<&Stmt> {
    let mut assigns = Vec::new();
    for stmt in body {
        match stmt {
            Stmt::Assign { .. } => assigns.push(stmt),
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    assigns.extend(collect_assigns(&branch.body));
                }
                assigns.extend(collect_assigns(else_body));
            }
            Stmt::While { body, .. } => assigns.extend(collect_assigns(body)),
            _ => {}
        }
    }
    assigns
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_a_direct_assignment_to_an_autoreadonly_property() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    a = 0.2\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[error]"));
        assert!(diagnostics[0].message.contains("'a'"));
    }

    #[test]
    fn flags_a_self_qualified_assignment() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    Self.a = 0.2\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
    }

    #[test]
    fn flags_a_compound_assignment() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    a += 1.0\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn matches_property_name_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property MyValue = 0.1 AutoReadOnly\n\nFunction Test()\n    myvalue = 0.2\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_a_plain_auto_property() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a Auto\n\nFunction Test()\n    a = 0.2\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_property_with_no_matching_write() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    Float b = a\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_local_variable_shadowing_the_property_name() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    Float a = 0.2\n    a = 0.3\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_parameter_shadowing_the_property_name() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test(Float a)\n    a = 0.2\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn still_flags_a_self_qualified_write_even_when_shadowed_by_a_local() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    Float a = 0.2\n    Self.a = 0.3\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 7);
    }

    #[test]
    fn flags_assignment_inside_if_block() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    If true\n        a = 0.2\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 7);
    }

    #[test]
    fn flags_assignment_inside_while_loop() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    While true\n        a = 0.2\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 7);
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nState Active\n    Function Test()\n        a = 0.2\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_a_member_access_on_an_unrelated_object() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test(SomeQuest quest)\n    quest.a = 0.2\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn returns_no_diagnostics_when_no_property_is_autoreadonly() {
        let diagnostics = check(
            "ScriptName Example\n\nFloat Property a Auto\n\nFunction Test()\n    Int i = 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }
}
